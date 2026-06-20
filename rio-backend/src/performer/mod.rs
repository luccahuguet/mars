pub mod handler;
mod osc;
pub mod parser;

use crate::crosswords::Crosswords;
use crate::event::sync::FairMutex;
use crate::event::RioEvent;
use crate::event::{EventListener, Msg, WindowId};
use corcovado::channel;
#[cfg(unix)]
use corcovado::unix::UnixReady;
use corcovado::{self, Events, PollOpt, Ready};
use std::borrow::Cow;
use std::collections::VecDeque;
use std::ffi::OsStr;
use std::io::{self, ErrorKind, Read, Write};
use std::sync::Arc;
use std::thread::{self, Builder, JoinHandle};
use std::time::{Duration, Instant};
use tracing::error;

/// Like `thread::spawn`, but with a `name` argument.
pub fn spawn_named<F, T, S>(name: S, f: F) -> JoinHandle<T>
where
    F: FnOnce() -> T + Send + 'static,
    T: Send + 'static,
    S: Into<String>,
{
    Builder::new()
        .name(name.into())
        .spawn(f)
        .expect("thread spawn works")
}

const READ_BUFFER_SIZE: usize = 0x10_0000;
/// Max bytes to read from the PTY while the terminal is locked.
const MAX_LOCKED_READ: usize = 1024;
const RENDER_BACKPRESSURE_TIMEOUT: Duration = Duration::from_millis(16);
const PERF_TRACE_ENV: &str = "MARS_PERF_TRACE";
const PTY_PERF_REPORT_INTERVAL: Duration = Duration::from_secs(1);
const NANOS_PER_SECOND: u128 = 1_000_000_000;

#[derive(Default)]
struct PtyPerfCounters {
    read_calls: u64,
    read_bytes: u64,
    read_zero_count: u64,
    read_would_block_count: u64,
    read_interrupted_count: u64,
    terminal_lock_miss_count: u64,
    forced_terminal_lock_count: u64,
    forced_terminal_lock_wait_ns: u128,
    forced_terminal_lock_wait_max_ns: u128,
    parser_batch_count: u64,
    parser_bytes: u64,
    parser_duration_ns: u128,
    parser_duration_max_ns: u128,
    parser_batch_max_bytes: usize,
    max_unprocessed_bytes: usize,
    max_locked_processed_bytes: usize,
    damage_check_count: u64,
    damage_sent_count: u64,
    damage_skipped_sync_count: u64,
    damage_skipped_in_flight_count: u64,
    damage_skipped_no_damage_count: u64,
    render_backpressure_pause_count: u64,
    render_backpressure_timeout_count: u64,
}

impl PtyPerfCounters {
    fn is_empty(&self) -> bool {
        self.read_calls == 0
            && self.terminal_lock_miss_count == 0
            && self.parser_batch_count == 0
            && self.damage_check_count == 0
            && self.damage_skipped_sync_count == 0
            && self.render_backpressure_pause_count == 0
            && self.render_backpressure_timeout_count == 0
    }
}

struct PtyPerfTrace {
    enabled: bool,
    route_id: usize,
    start: Instant,
    last_report: Instant,
    report_index: u64,
    counters: PtyPerfCounters,
}

impl PtyPerfTrace {
    fn from_env(route_id: usize) -> Self {
        let enabled = std::env::var_os(PERF_TRACE_ENV)
            .as_deref()
            .is_some_and(perf_trace_enables_pty);
        let now = Instant::now();
        let trace = Self {
            enabled,
            route_id,
            start: now,
            last_report: now,
            report_index: 0,
            counters: PtyPerfCounters::default(),
        };

        if enabled {
            eprintln!(
                "{{\"event\":\"pty_trace_start\",\"pid\":{},\"route_id\":{},\"elapsed_ns\":0}}",
                std::process::id(),
                route_id
            );
        }

        trace
    }

    #[inline]
    fn now(&self) -> Option<Instant> {
        self.enabled.then(Instant::now)
    }

    #[inline]
    fn record_read_bytes(&mut self, bytes: usize, unprocessed: usize) {
        if !self.enabled {
            return;
        }

        self.counters.read_calls += 1;
        self.counters.read_bytes += bytes as u64;
        self.record_unprocessed(unprocessed);
    }

    #[inline]
    fn record_read_zero(&mut self, unprocessed: usize) {
        if !self.enabled {
            return;
        }

        self.counters.read_calls += 1;
        self.counters.read_zero_count += 1;
        self.record_unprocessed(unprocessed);
    }

    #[inline]
    fn record_read_would_block(&mut self, unprocessed: usize) {
        if !self.enabled {
            return;
        }

        self.counters.read_calls += 1;
        self.counters.read_would_block_count += 1;
        self.record_unprocessed(unprocessed);
    }

    #[inline]
    fn record_read_interrupted(&mut self, unprocessed: usize) {
        if !self.enabled {
            return;
        }

        self.counters.read_calls += 1;
        self.counters.read_interrupted_count += 1;
        self.record_unprocessed(unprocessed);
    }

    #[inline]
    fn record_lock_miss(&mut self, unprocessed: usize) {
        if !self.enabled {
            return;
        }

        self.counters.terminal_lock_miss_count += 1;
        self.record_unprocessed(unprocessed);
        self.maybe_report();
    }

    #[inline]
    fn record_forced_terminal_lock(&mut self, started_at: Option<Instant>) {
        if !self.enabled {
            return;
        }

        let Some(started_at) = started_at else {
            return;
        };
        let wait_ns = started_at.elapsed().as_nanos();
        self.counters.forced_terminal_lock_count += 1;
        self.counters.forced_terminal_lock_wait_ns += wait_ns;
        self.counters.forced_terminal_lock_wait_max_ns =
            self.counters.forced_terminal_lock_wait_max_ns.max(wait_ns);
    }

    #[inline]
    fn record_parse(
        &mut self,
        bytes: usize,
        locked_processed: usize,
        started_at: Option<Instant>,
    ) {
        if !self.enabled {
            return;
        }

        let Some(started_at) = started_at else {
            return;
        };
        let duration_ns = started_at.elapsed().as_nanos();
        self.counters.parser_batch_count += 1;
        self.counters.parser_bytes += bytes as u64;
        self.counters.parser_duration_ns += duration_ns;
        self.counters.parser_duration_max_ns =
            self.counters.parser_duration_max_ns.max(duration_ns);
        self.counters.parser_batch_max_bytes =
            self.counters.parser_batch_max_bytes.max(bytes);
        self.counters.max_locked_processed_bytes = self
            .counters
            .max_locked_processed_bytes
            .max(locked_processed);
        self.maybe_report();
    }

    #[inline]
    fn record_damage_sync_skipped(&mut self) {
        if self.enabled {
            self.counters.damage_skipped_sync_count += 1;
        }
    }

    #[inline]
    fn record_damage_sent(&mut self) {
        if self.enabled {
            self.counters.damage_check_count += 1;
            self.counters.damage_sent_count += 1;
        }
    }

    #[inline]
    fn record_damage_in_flight_skipped(&mut self) {
        if self.enabled {
            self.counters.damage_check_count += 1;
            self.counters.damage_skipped_in_flight_count += 1;
        }
    }

    #[inline]
    fn record_damage_no_damage_skipped(&mut self) {
        if self.enabled {
            self.counters.damage_check_count += 1;
            self.counters.damage_skipped_no_damage_count += 1;
        }
    }

    #[inline]
    fn record_render_backpressure_pause(&mut self) {
        if self.enabled {
            self.counters.render_backpressure_pause_count += 1;
            self.maybe_report();
        }
    }

    #[inline]
    fn record_render_backpressure_timeout(&mut self) {
        if self.enabled {
            self.counters.render_backpressure_timeout_count += 1;
            self.maybe_report();
        }
    }

    #[inline]
    fn record_unprocessed(&mut self, unprocessed: usize) {
        self.counters.max_unprocessed_bytes =
            self.counters.max_unprocessed_bytes.max(unprocessed);
    }

    fn maybe_report(&mut self) {
        if !self.enabled {
            return;
        }

        let now = Instant::now();
        if now.duration_since(self.last_report) >= PTY_PERF_REPORT_INTERVAL {
            self.report(now, false);
        }
    }

    fn finish(&mut self) {
        if self.enabled {
            self.report(Instant::now(), true);
        }
    }

    fn report(&mut self, now: Instant, final_report: bool) {
        let interval = now.duration_since(self.last_report);
        let counters = std::mem::take(&mut self.counters);
        if counters.is_empty() && !final_report {
            self.last_report = now;
            return;
        }

        let interval_ns = interval.as_nanos();
        let elapsed_ns = now.duration_since(self.start).as_nanos();
        eprintln!(
            concat!(
                "{{\"event\":\"pty_perf\",\"pid\":{},\"route_id\":{},",
                "\"report_index\":{},\"final\":{},\"elapsed_ns\":{},",
                "\"interval_ns\":{},\"read_calls\":{},\"read_bytes\":{},",
                "\"read_bytes_per_s\":{},\"read_zero_count\":{},",
                "\"read_would_block_count\":{},\"read_interrupted_count\":{},",
                "\"terminal_lock_miss_count\":{},",
                "\"forced_terminal_lock_count\":{},",
                "\"forced_terminal_lock_wait_ns\":{},",
                "\"forced_terminal_lock_wait_max_ns\":{},",
                "\"parser_batch_count\":{},\"parser_batches_per_s\":{},",
                "\"parser_bytes\":{},\"parser_bytes_per_s\":{},",
                "\"parser_duration_ns\":{},\"parser_duration_avg_ns\":{},",
                "\"parser_duration_max_ns\":{},\"parser_batch_avg_bytes\":{},",
                "\"parser_batch_max_bytes\":{},\"max_unprocessed_bytes\":{},",
                "\"max_locked_processed_bytes\":{},\"damage_check_count\":{},",
                "\"damage_sent_count\":{},\"damage_skipped_sync_count\":{},",
                "\"damage_skipped_in_flight_count\":{},",
                "\"damage_skipped_no_damage_count\":{},",
                "\"render_backpressure_pause_count\":{},",
                "\"render_backpressure_timeout_count\":{}}}"
            ),
            std::process::id(),
            self.route_id,
            self.report_index,
            final_report,
            elapsed_ns,
            interval_ns,
            counters.read_calls,
            counters.read_bytes,
            per_second(counters.read_bytes, interval_ns),
            counters.read_zero_count,
            counters.read_would_block_count,
            counters.read_interrupted_count,
            counters.terminal_lock_miss_count,
            counters.forced_terminal_lock_count,
            counters.forced_terminal_lock_wait_ns,
            counters.forced_terminal_lock_wait_max_ns,
            counters.parser_batch_count,
            per_second(counters.parser_batch_count, interval_ns),
            counters.parser_bytes,
            per_second(counters.parser_bytes, interval_ns),
            counters.parser_duration_ns,
            average(counters.parser_duration_ns, counters.parser_batch_count),
            counters.parser_duration_max_ns,
            average(counters.parser_bytes as u128, counters.parser_batch_count),
            counters.parser_batch_max_bytes,
            counters.max_unprocessed_bytes,
            counters.max_locked_processed_bytes,
            counters.damage_check_count,
            counters.damage_sent_count,
            counters.damage_skipped_sync_count,
            counters.damage_skipped_in_flight_count,
            counters.damage_skipped_no_damage_count,
            counters.render_backpressure_pause_count,
            counters.render_backpressure_timeout_count
        );

        self.last_report = now;
        self.report_index += 1;
    }
}

fn perf_trace_enables_pty(value: &OsStr) -> bool {
    value
        .to_string_lossy()
        .split(perf_trace_separator)
        .any(|token| {
            matches!(
                token.trim().to_ascii_lowercase().as_str(),
                "pty" | "all" | "1" | "true" | "yes" | "on"
            )
        })
}

fn perf_trace_separator(ch: char) -> bool {
    ch == ',' || ch == ';' || ch == ':' || ch.is_ascii_whitespace()
}

fn per_second(value: u64, interval_ns: u128) -> u128 {
    if interval_ns == 0 {
        0
    } else {
        u128::from(value) * NANOS_PER_SECOND / interval_ns
    }
}

fn average(total: u128, count: u64) -> u128 {
    if count == 0 {
        0
    } else {
        total / u128::from(count)
    }
}

fn next_timeout(a: Option<Instant>, b: Option<Instant>) -> Option<Duration> {
    a.into_iter()
        .chain(b)
        .min()
        .map(|deadline| deadline.saturating_duration_since(Instant::now()))
}

struct PeekableReceiver<T> {
    rx: channel::Receiver<T>,
    peeked: Option<T>,
}

impl<T> PeekableReceiver<T> {
    fn new(rx: channel::Receiver<T>) -> Self {
        Self { rx, peeked: None }
    }

    fn peek(&mut self) -> Option<&T> {
        if self.peeked.is_none() {
            self.peeked = self.rx.try_recv().ok();
        }

        self.peeked.as_ref()
    }

    fn recv(&mut self) -> Option<T> {
        if self.peeked.is_some() {
            self.peeked.take()
        } else {
            self.rx.try_recv().ok()
        }
    }
}

pub struct Machine<T: teletypewriter::EventedPty, U: EventListener> {
    sender: channel::Sender<Msg>,
    receiver: PeekableReceiver<Msg>,
    pty: T,
    poll: corcovado::Poll,
    terminal: Arc<FairMutex<Crosswords<U>>>,
    event_proxy: U,
    window_id: WindowId,
    route_id: usize,
}

#[derive(Default)]
pub struct State {
    write_list: VecDeque<Cow<'static, [u8]>>,
    writing: Option<Writing>,
    parser: handler::Processor,
    pty_read_paused_until: Option<Instant>,
}

impl State {
    #[inline]
    fn ensure_next(&mut self) {
        if self.writing.is_none() {
            self.goto_next();
        }
    }

    #[inline]
    fn goto_next(&mut self) {
        self.writing = self.write_list.pop_front().map(Writing::new);
    }

    #[inline]
    fn take_current(&mut self) -> Option<Writing> {
        self.writing.take()
    }

    #[inline]
    fn needs_write(&self) -> bool {
        self.writing.is_some() || !self.write_list.is_empty()
    }

    #[inline]
    fn pty_read_paused(&self) -> bool {
        self.pty_read_paused_until.is_some()
    }

    #[inline]
    fn pause_pty_read_for_render(&mut self) {
        self.pty_read_paused_until = Some(Instant::now() + RENDER_BACKPRESSURE_TIMEOUT);
    }

    #[inline]
    fn resume_pty_read(&mut self) {
        self.pty_read_paused_until = None;
    }

    #[inline]
    fn pty_read_pause_deadline(&self) -> Option<Instant> {
        self.pty_read_paused_until
    }

    #[inline]
    fn resume_pty_read_if_due(&mut self, now: Instant) -> bool {
        if self
            .pty_read_paused_until
            .is_some_and(|deadline| now >= deadline)
        {
            self.resume_pty_read();
            true
        } else {
            false
        }
    }

    #[inline]
    fn set_current(&mut self, new: Option<Writing>) {
        self.writing = new;
    }
}

struct Writing {
    source: Cow<'static, [u8]>,
    written: usize,
}

impl Writing {
    #[inline]
    fn new(c: Cow<'static, [u8]>) -> Writing {
        Writing {
            source: c,
            written: 0,
        }
    }

    #[inline]
    fn advance(&mut self, n: usize) {
        self.written += n;
    }

    #[inline]
    fn remaining_bytes(&self) -> &[u8] {
        &self.source[self.written..]
    }

    #[inline]
    fn finished(&self) -> bool {
        self.written >= self.source.len()
    }
}

impl<T, U> Machine<T, U>
where
    T: teletypewriter::EventedPty + Send + 'static,
    U: EventListener + Send + 'static,
{
    pub fn new(
        terminal: Arc<FairMutex<Crosswords<U>>>,
        pty: T,
        event_proxy: U,
        window_id: WindowId,
        route_id: usize,
    ) -> Result<Machine<T, U>, Box<dyn std::error::Error>> {
        let (sender, receiver) = channel::channel();
        let poll = corcovado::Poll::new()?;

        Ok(Machine {
            sender,
            receiver: PeekableReceiver::new(receiver),
            poll,
            pty,
            terminal,
            event_proxy,
            window_id,
            route_id,
        })
    }

    #[inline]
    fn pty_read(
        &mut self,
        state: &mut State,
        buf: &mut [u8],
        perf_trace: &mut PtyPerfTrace,
    ) -> io::Result<()> {
        let mut unprocessed = 0;
        let mut processed = 0;
        let read_limit = MAX_LOCKED_READ.min(buf.len());

        // Reserve the next terminal lock for PTY reading.
        let _terminal_lease = Some(self.terminal.lease());
        let mut terminal = None;

        loop {
            // Read from the PTY.
            if unprocessed < read_limit {
                match self.pty.reader().read(&mut buf[unprocessed..read_limit]) {
                    // This is received on Windows/macOS when no more data is readable from the PTY.
                    Ok(0) if unprocessed == 0 => {
                        perf_trace.record_read_zero(unprocessed);
                        break;
                    }
                    Ok(got) => {
                        if got == 0 {
                            perf_trace.record_read_zero(unprocessed);
                        } else {
                            unprocessed += got;
                            perf_trace.record_read_bytes(got, unprocessed);
                        }
                    }
                    Err(err) => match err.kind() {
                        ErrorKind::Interrupted => {
                            perf_trace.record_read_interrupted(unprocessed);
                            // Go back to mio if we're caught up on parsing and the PTY would block.
                            if unprocessed == 0 {
                                break;
                            }
                        }
                        ErrorKind::WouldBlock => {
                            perf_trace.record_read_would_block(unprocessed);
                            // Go back to mio if we're caught up on parsing and the PTY would block.
                            if unprocessed == 0 {
                                break;
                            }
                        }
                        _ => return Err(err),
                    },
                }
            }

            // Attempt to lock the terminal.
            let terminal = match &mut terminal {
                Some(terminal) => terminal,
                None => terminal.insert(match self.terminal.try_lock_unfair() {
                    // Force block once this read cycle has accumulated a
                    // full parser batch; otherwise keep collecting bytes
                    // without taking the terminal lock.
                    None if unprocessed >= read_limit => {
                        perf_trace.record_lock_miss(unprocessed);
                        let lock_wait_start = perf_trace.now();
                        let terminal = self.terminal.lock_unfair();
                        perf_trace.record_forced_terminal_lock(lock_wait_start);
                        terminal
                    }
                    None => {
                        perf_trace.record_lock_miss(unprocessed);
                        continue;
                    }
                    Some(terminal) => terminal,
                }),
            };

            // Parse the incoming bytes.
            let parse_bytes = unprocessed;
            let parse_start = perf_trace.now();
            state.parser.advance(&mut **terminal, &buf[..unprocessed]);

            processed += unprocessed;
            unprocessed = 0;
            perf_trace.record_parse(parse_bytes, processed, parse_start);

            // Assure we're not blocking the terminal too long unnecessarily.
            if processed >= MAX_LOCKED_READ {
                break;
            }
        }

        // Notify renderer that new damage is available.
        // Only send if no event is already in flight — the renderer will
        // extract all accumulated damage when it locks the terminal.
        let mut should_pause_after_parse = false;
        if processed > 0 {
            if state.parser.sync_pending() {
                // Synchronized output defers render notifications, but it
                // still performs parser/model work. Backpressure it too.
                perf_trace.record_damage_sync_skipped();
                should_pause_after_parse = true;
            } else {
                if let Some(ref mut term) = terminal {
                    should_pause_after_parse =
                        self.notify_renderer_if_damaged(&mut **term, perf_trace);
                }
            }
        }
        drop(terminal);
        if should_pause_after_parse {
            state.pause_pty_read_for_render();
            perf_trace.record_render_backpressure_pause();
            thread::yield_now();
        }

        Ok(())
    }

    /// Drain the channel.
    ///
    /// Returns `false` when a shutdown message was received.
    fn drain_recv_channel(&mut self, state: &mut State) -> bool {
        while let Some(msg) = self.receiver.recv() {
            match msg {
                Msg::Input(input) => state.write_list.push_back(input),
                Msg::TerminalDamageConsumed => state.resume_pty_read(),
                Msg::Resize(window_size) => {
                    let _ = self.pty.set_winsize(window_size);
                }
                Msg::Shutdown => return false,
            }
        }

        true
    }

    /// Returns a `bool` indicating whether or not the event loop should continue running.
    #[inline]
    fn channel_event(&mut self, token: corcovado::Token, state: &mut State) -> bool {
        if !self.drain_recv_channel(state) {
            return false;
        }

        self.poll
            .reregister(
                &self.receiver.rx,
                token,
                Ready::readable(),
                PollOpt::edge() | PollOpt::oneshot(),
            )
            .unwrap();

        true
    }

    #[inline]
    fn pty_write(&mut self, state: &mut State) -> io::Result<()> {
        state.ensure_next();

        'write_many: while let Some(mut current) = state.take_current() {
            'write_one: loop {
                match self.pty.writer().write(current.remaining_bytes()) {
                    Ok(0) => {
                        state.set_current(Some(current));
                        break 'write_many;
                    }
                    Ok(n) => {
                        current.advance(n);
                        if current.finished() {
                            state.goto_next();
                            break 'write_one;
                        }
                    }
                    Err(err) => {
                        state.set_current(Some(current));
                        match err.kind() {
                            ErrorKind::Interrupted | ErrorKind::WouldBlock => {
                                break 'write_many
                            }
                            _ => return Err(err),
                        }
                    }
                }
            }
        }
        Ok(())
    }

    pub fn channel(&self) -> channel::Sender<Msg> {
        self.sender.clone()
    }

    #[inline]
    fn notify_renderer_if_damaged(
        &self,
        terminal: &mut Crosswords<U>,
        perf_trace: &mut PtyPerfTrace,
    ) -> bool {
        if terminal.peek_damage_event().is_none() {
            perf_trace.record_damage_no_damage_skipped();
            return false;
        }

        if terminal.damage_event_in_flight {
            perf_trace.record_damage_in_flight_skipped();
        } else {
            terminal.damage_event_in_flight = true;
            self.event_proxy
                .send_event(RioEvent::TerminalDamaged(self.route_id), self.window_id);
            perf_trace.record_damage_sent();
        }

        true
    }

    pub fn spawn(mut self) -> JoinHandle<(Self, State)> {
        spawn_named("PTY reader", move || {
            let mut state = State::default();
            let mut buf = [0u8; READ_BUFFER_SIZE];
            let mut pty_perf_trace = PtyPerfTrace::from_env(self.route_id);

            let mut tokens = (0..).map(Into::into);

            let poll_opts = PollOpt::edge() | PollOpt::oneshot();

            let channel_token = tokens.next().unwrap();
            self.poll
                .register(
                    &self.receiver.rx,
                    channel_token,
                    Ready::readable(),
                    poll_opts,
                )
                .unwrap();

            // Register TTY through EventedRW interface.
            self.pty
                .register(&self.poll, &mut tokens, Ready::readable(), poll_opts)
                .unwrap();

            let mut events = Events::with_capacity(1024);

            'event_loop: loop {
                // Wakeup the event loop when a synchronized update timeout was reached.
                let sync_timeout = state.parser.sync_timeout().sync_timeout();
                let timeout = next_timeout(sync_timeout, state.pty_read_pause_deadline());

                events.clear();
                if let Err(err) = self.poll.poll(&mut events, timeout) {
                    match err.kind() {
                        ErrorKind::Interrupted => continue,
                        _ => {
                            error!("Event loop polling error: {err}");
                            break 'event_loop;
                        }
                    }
                }

                // Handle synchronized update and PTY backpressure timeouts.
                if events.is_empty() && self.receiver.peek().is_none() {
                    let now = Instant::now();
                    if state.resume_pty_read_if_due(now) {
                        pty_perf_trace.record_render_backpressure_timeout();
                    }

                    if sync_timeout.is_some_and(|deadline| now >= deadline) {
                        let mut terminal = self.terminal.lock();
                        state.parser.stop_sync(&mut *terminal);
                        let should_yield_after_damage = self.notify_renderer_if_damaged(
                            &mut *terminal,
                            &mut pty_perf_trace,
                        );
                        drop(terminal);

                        if should_yield_after_damage {
                            state.pause_pty_read_for_render();
                            pty_perf_trace.record_render_backpressure_pause();
                            thread::yield_now();
                        }
                    }
                }

                // Handle channel events, if there are any.
                if !self.drain_recv_channel(&mut state) {
                    break;
                }

                for event in events.iter() {
                    match event.token() {
                        token if token == channel_token => {
                            // In case should shutdown by message
                            if !self.channel_event(channel_token, &mut state) {
                                break 'event_loop;
                            }
                        }
                        token if token == self.pty.child_event_token() => {
                            if let Some(teletypewriter::ChildEvent::Exited) =
                                self.pty.next_child_event()
                            {
                                // In the future allow configure exit
                                // if self.hold {
                                //     With hold enabled, make sure the PTY is drained.
                                //     let _ = self.pty_read(&mut state, &mut buf);
                                // } else {
                                //     // Without hold, shutdown the terminal.
                                //     self.terminal.lock().exit();
                                // }

                                self.terminal.lock().exit();

                                self.event_proxy
                                    .send_event(RioEvent::Render, self.window_id);

                                break 'event_loop;
                            }
                        }

                        token
                            if token == self.pty.read_token()
                                || token == self.pty.write_token() =>
                        {
                            #[cfg(unix)]
                            if UnixReady::from(event.readiness()).is_hup() {
                                // Don't try to do I/O on a dead PTY.
                                continue;
                            }
                            if event.readiness().is_readable() {
                                if !state.pty_read_paused() {
                                    if let Err(err) = self.pty_read(
                                        &mut state,
                                        &mut buf,
                                        &mut pty_perf_trace,
                                    ) {
                                        // On Linux, a `read` on the master side of a PTY can fail
                                        // with `EIO` if the client side hangs up.  In that case,
                                        // just loop back round for the inevitable `Exited` event.
                                        #[cfg(target_os = "linux")]
                                        if err.raw_os_error() == Some(libc::EIO) {
                                            continue;
                                        }

                                        error!(
                                            "Error reading from PTY in event loop: {}",
                                            err
                                        );
                                        break 'event_loop;
                                    }
                                }
                            }

                            if event.readiness().is_writable() {
                                if let Err(err) = self.pty_write(&mut state) {
                                    error!("Error writing to PTY in event loop: {}", err);
                                    break 'event_loop;
                                }
                            }
                        }
                        _ => (),
                    }
                }

                // Register read/write interest if necessary. With one-shot
                // polling, skipping the reregister is the least surprising
                // way to keep PTY reads paused without passing an empty
                // readiness mask down to the OS selector.
                let needs_write = state.needs_write();
                if !state.pty_read_paused() || needs_write {
                    let mut interest = Ready::empty();
                    if !state.pty_read_paused() {
                        interest.insert(Ready::readable());
                    }
                    if needs_write {
                        interest.insert(Ready::writable());
                    }

                    self.pty
                        .reregister(&self.poll, interest, poll_opts)
                        .unwrap();
                }
            }

            // The evented instances are not dropped here so deregister them explicitly.
            let _ = self.poll.deregister(&self.receiver.rx);
            let _ = self.pty.deregister(&self.poll);
            pty_perf_trace.finish();

            (self, state)
        })
    }
}
