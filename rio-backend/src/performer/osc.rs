//! Typed parsing helpers for OSC (Operating System Command) sequences.
//!
//! The Williams parser hands the dispatcher a `&[&[u8]]` of separator-split
//! parameter slices. Each helper here takes those raw slices and returns a
//! typed result for the corresponding OSC command, leaving the dispatcher in
//! `handler.rs` as a thin glue layer.

use std::str::FromStr;

use cursor_icon::CursorIcon;

use crate::ansi::CursorShape;
use crate::config::colors::{ColorRgb, NamedColor};
use crate::crosswords::square::Hyperlink;
use crate::event::{ProgressReport, ProgressState};
use crate::simd_utf8;

use super::handler::{
    KittyClipboard, KittyClipboardLocation, KittyClipboardOperation, KittyFileTransfer,
    KittyFileTransferAction, KittyNotification, KittyNotificationKind, SemanticPrompt,
    SemanticPromptAction, SemanticPromptClick, SemanticPromptKind, SemanticPromptRedraw,
    TextSizing, TextSizingAlign,
};

/// Either a concrete color value or a query for the current value.
pub(super) enum ColorSpec {
    Set(ColorRgb),
    Query,
}

pub(super) struct PaletteEntry {
    pub index: u8,
    pub spec: ColorSpec,
}

pub(super) struct DynamicColorEntry {
    pub index: NamedColor,
    pub dynamic_code: u16,
    pub spec: ColorSpec,
}

pub(super) enum KittyColorSpec {
    Set(ColorRgb),
    Query,
    Reset,
}

pub(super) struct KittyColorEntry {
    pub key: String,
    pub index: Option<usize>,
    pub spec: KittyColorSpec,
}

pub(super) enum MouseCursorOp {
    Set(Option<CursorIcon>),
    Push(Vec<CursorIcon>),
    Pop,
    Query(Vec<String>),
}

pub(super) enum ClipboardOp<'a> {
    Load { kind: u8 },
    Store { kind: u8, payload: &'a [u8] },
}

pub(super) enum PaletteReset {
    All,
    Indices(Vec<u8>),
}

fn parse_i32(input: &[u8]) -> Option<i32> {
    simd_utf8::from_utf8_fast(input).ok()?.parse().ok()
}

/// Parse an `xterm`-style color value (`#rgb`, `#rrggbb`, `rgb:r/g/b`,
/// `rgbi:r/g/b`, or a small set of standard color names).
pub(super) fn xparse_color(color: &[u8]) -> Option<ColorRgb> {
    if !color.is_empty() && color[0] == b'#' {
        parse_legacy_color(&color[1..])
    } else if color.len() >= 4 && &color[..4] == b"rgb:" {
        parse_rgb_color(&color[4..])
    } else if color.len() >= 5 && &color[..5] == b"rgbi:" {
        parse_rgbi_color(&color[5..])
    } else {
        parse_named_color(color)
    }
}

/// Parse colors in `rgb:r(rrr)/g(ggg)/b(bbb)` format.
fn parse_rgb_color(color: &[u8]) -> Option<ColorRgb> {
    let colors = simd_utf8::from_utf8_fast(color)
        .ok()?
        .split('/')
        .collect::<Vec<_>>();

    if colors.len() != 3 {
        return None;
    }

    // Scale values instead of filling with `0`s.
    let scale = |input: &str| {
        if input.len() > 4 {
            None
        } else {
            let max = u32::pow(16, input.len() as u32) - 1;
            let value = u32::from_str_radix(input, 16).ok()?;
            Some((255 * value / max) as u8)
        }
    };

    Some(ColorRgb {
        r: scale(colors[0])?,
        g: scale(colors[1])?,
        b: scale(colors[2])?,
    })
}

/// Parse colors in `rgbi:r/g/b` format, where each channel is 0.0..=1.0.
fn parse_rgbi_color(color: &[u8]) -> Option<ColorRgb> {
    let colors = simd_utf8::from_utf8_fast(color)
        .ok()?
        .split('/')
        .collect::<Vec<_>>();

    if colors.len() != 3 {
        return None;
    }

    let scale = |input: &str| {
        let value = input.parse::<f32>().ok()?;
        if !(0.0..=1.0).contains(&value) {
            return None;
        }
        Some((value * 255.0).round() as u8)
    };

    Some(ColorRgb {
        r: scale(colors[0])?,
        g: scale(colors[1])?,
        b: scale(colors[2])?,
    })
}

fn parse_named_color(color: &[u8]) -> Option<ColorRgb> {
    let color = simd_utf8::from_utf8_fast(color).ok()?;
    let rgb = match color.to_ascii_lowercase().as_str() {
        "black" => (0x00, 0x00, 0x00),
        "red" => (0xff, 0x00, 0x00),
        "green" => (0x00, 0x80, 0x00),
        "yellow" => (0xff, 0xff, 0x00),
        "blue" => (0x00, 0x00, 0xff),
        "magenta" | "fuchsia" => (0xff, 0x00, 0xff),
        "cyan" | "aqua" => (0x00, 0xff, 0xff),
        "white" => (0xff, 0xff, 0xff),
        "gray" | "grey" => (0x80, 0x80, 0x80),
        "aliceblue" => (0xf0, 0xf8, 0xff),
        _ => return None,
    };

    Some(ColorRgb {
        r: rgb.0,
        g: rgb.1,
        b: rgb.2,
    })
}

/// Parse colors in `#r(rrr)g(ggg)b(bbb)` format.
fn parse_legacy_color(color: &[u8]) -> Option<ColorRgb> {
    let item_len = color.len() / 3;

    // Truncate/Fill to two byte precision.
    let color_from_slice = |slice: &[u8]| {
        let col =
            usize::from_str_radix(simd_utf8::from_utf8_fast(slice).ok()?, 16).ok()? << 4;
        Some((col >> (4 * slice.len().saturating_sub(1))) as u8)
    };

    Some(ColorRgb {
        r: color_from_slice(&color[0..item_len])?,
        g: color_from_slice(&color[item_len..item_len * 2])?,
        b: color_from_slice(&color[item_len * 2..])?,
    })
}

pub(super) fn parse_number(input: &[u8]) -> Option<u8> {
    if input.is_empty() {
        return None;
    }
    let mut num: u8 = 0;
    for c in input {
        let c = *c as char;
        if let Some(digit) = c.to_digit(10) {
            num = num
                .checked_mul(10)
                .and_then(|v| v.checked_add(digit as u8))?
        } else {
            return None;
        }
    }
    Some(num)
}

/// OSC 0 / OSC 2: window title set as `;`-joined params.
pub(super) fn parse_title(params: &[&[u8]]) -> Option<String> {
    if params.len() < 2 {
        return None;
    }
    Some(
        params[1..]
            .iter()
            .flat_map(|x| simd_utf8::from_utf8_fast(x))
            .collect::<Vec<&str>>()
            .join(";")
            .trim()
            .to_owned(),
    )
}

/// OSC 4: a list of `(index, color | "?")` pairs in `params[1..]`.
pub(super) fn parse_palette_entries(params: &[&[u8]]) -> Option<Vec<PaletteEntry>> {
    if params.len() <= 1 || params.len().is_multiple_of(2) {
        return None;
    }

    let mut out = Vec::with_capacity(params.len() / 2);
    for chunk in params[1..].chunks(2) {
        let index = parse_number(chunk[0])?;
        let spec = if chunk[1] == b"?" {
            ColorSpec::Query
        } else {
            ColorSpec::Set(xparse_color(chunk[1])?)
        };
        out.push(PaletteEntry { index, spec });
    }
    Some(out)
}

/// OSC 21: Kitty keyed color protocol.
pub(super) fn parse_kitty_color_entries(
    params: &[&[u8]],
) -> Option<Vec<KittyColorEntry>> {
    if params.len() < 2 {
        return None;
    }

    let mut out = Vec::with_capacity(params.len() - 1);
    for param in &params[1..] {
        if param.is_empty() {
            continue;
        }

        let (key, value) = match param.iter().position(|b| *b == b'=') {
            Some(position) => (&param[..position], Some(&param[position + 1..])),
            None => (*param, None),
        };
        if key.is_empty() {
            continue;
        }

        let key = simd_utf8::from_utf8_fast(key).ok()?.to_owned();
        let index = kitty_color_index(&key);
        let spec = match value {
            Some(value) if value == b"?" => KittyColorSpec::Query,
            Some(value) if value.is_empty() => KittyColorSpec::Reset,
            Some(value) => match xparse_color(value) {
                Some(color) => KittyColorSpec::Set(color),
                None => continue,
            },
            None => KittyColorSpec::Reset,
        };

        out.push(KittyColorEntry { key, index, spec });
    }

    Some(out)
}

fn kitty_color_index(key: &str) -> Option<usize> {
    match key {
        "foreground" => Some(NamedColor::Foreground as usize),
        "background" => Some(NamedColor::Background as usize),
        "cursor" => Some(NamedColor::Cursor as usize),
        "cursor_text" => Some(NamedColor::CursorText as usize),
        "selection_foreground" => Some(NamedColor::SelectionForeground as usize),
        "selection_background" => Some(NamedColor::SelectionBackground as usize),
        "visual_bell" => Some(NamedColor::VisualBell as usize),
        "second_transparent_background" | "transparent_background" => {
            Some(NamedColor::TransparentBackground as usize)
        }
        _ => key.parse::<u8>().ok().map(usize::from),
    }
}

/// OSC 7: working directory as a `file://` URL.
pub(super) fn parse_current_directory(param: &[u8]) -> Option<String> {
    let s = simd_utf8::from_utf8_fast(param).ok()?;
    let url = url::Url::parse(s).ok()?;
    let path = url.path();

    // The URL crate prepends a leading slash on Windows paths; strip it.
    #[cfg(windows)]
    let path = path.strip_prefix('/').unwrap_or(path);

    Some(path.to_owned())
}

/// OSC 8: extract `id=...` from `key=val:key=val` link params.
pub(super) fn parse_hyperlink_id(link_params: &[u8]) -> Option<&str> {
    link_params
        .split(|&b| b == b':')
        .find_map(|kv| kv.strip_prefix(b"id="))
        .and_then(|kv| simd_utf8::from_utf8_fast(kv).ok())
}

/// Construct a [`Hyperlink`] from the link params + URI bytes. Returns
/// `None` for an empty URI (caller should clear the active hyperlink).
pub(super) fn parse_hyperlink(link_params: &[u8], uri_param: &[u8]) -> Option<Hyperlink> {
    let uri = simd_utf8::from_utf8_fast(uri_param).unwrap_or_default();
    if uri.is_empty() {
        return None;
    }
    Some(Hyperlink::new(parse_hyperlink_id(link_params), uri))
}

/// OSC 9;4 — ConEmu/Windows-Terminal progress reporting.
/// Format: `9;4;<state>;<progress>` (progress optional).
pub(super) fn parse_progress_report(params: &[&[u8]]) -> Option<ProgressReport> {
    if params.len() < 3 || params[1] != b"4" {
        return None;
    }
    let state = match params[2] {
        b"0" => ProgressState::Remove,
        b"1" => ProgressState::Set,
        b"2" => ProgressState::Error,
        b"3" => ProgressState::Indeterminate,
        b"4" => ProgressState::Pause,
        _ => return None,
    };
    let progress = if params.len() >= 4 {
        parse_number(params[3]).map(|p| p.min(100))
    } else {
        None
    };
    Some(ProgressReport { state, progress })
}

/// OSC 133 — semantic prompt and command-zone markers.
pub(super) fn parse_semantic_prompt(params: &[&[u8]]) -> Option<SemanticPrompt> {
    let action = match *params.get(1)? {
        b"L" => SemanticPromptAction::FreshLine,
        b"A" => SemanticPromptAction::FreshLineNewPrompt,
        b"N" => SemanticPromptAction::NewCommand,
        b"P" => SemanticPromptAction::PromptStart,
        b"B" => SemanticPromptAction::EndPromptStartInput,
        b"I" => SemanticPromptAction::EndPromptStartInputTerminateEol,
        b"C" => SemanticPromptAction::EndInputStartOutput,
        b"D" => SemanticPromptAction::EndCommand,
        _ => return None,
    };

    let mut prompt = SemanticPrompt::new(action);
    for (idx, option) in params.iter().skip(2).enumerate() {
        if matches!(action, SemanticPromptAction::EndCommand)
            && idx == 0
            && !option.contains(&b'=')
        {
            prompt.exit_code = parse_i32(option);
            continue;
        }

        let Some(eq_idx) = option.iter().position(|b| *b == b'=') else {
            continue;
        };
        let key = &option[..eq_idx];
        let value = &option[eq_idx + 1..];

        match key {
            b"aid" => prompt.aid = Some(simd_utf8::from_utf8_lossy_fast(value)),
            b"k" => prompt.prompt_kind = parse_semantic_prompt_kind(value),
            b"cl" => prompt.click = parse_semantic_prompt_click(value),
            b"click_events" => prompt.click_events = parse_bool(value),
            b"redraw" => prompt.redraw = parse_semantic_prompt_redraw(value),
            b"cmdline" => {
                prompt.command_line = Some(simd_utf8::from_utf8_lossy_fast(value))
            }
            b"cmdline_url" => {
                prompt.command_line_url = Some(simd_utf8::from_utf8_lossy_fast(value))
            }
            b"err" => prompt.error = Some(simd_utf8::from_utf8_lossy_fast(value)),
            _ => {}
        }
    }

    Some(prompt)
}

fn parse_bool(input: &[u8]) -> Option<bool> {
    match input {
        b"0" => Some(false),
        b"1" => Some(true),
        _ => None,
    }
}

fn parse_semantic_prompt_kind(input: &[u8]) -> Option<SemanticPromptKind> {
    match input {
        b"i" => Some(SemanticPromptKind::Initial),
        b"r" => Some(SemanticPromptKind::Right),
        b"c" => Some(SemanticPromptKind::Continuation),
        b"s" => Some(SemanticPromptKind::Secondary),
        _ => None,
    }
}

fn parse_semantic_prompt_click(input: &[u8]) -> Option<SemanticPromptClick> {
    match input {
        b"line" => Some(SemanticPromptClick::Line),
        b"m" => Some(SemanticPromptClick::Multiple),
        b"v" => Some(SemanticPromptClick::ConservativeVertical),
        b"w" => Some(SemanticPromptClick::SmartVertical),
        _ => None,
    }
}

fn parse_semantic_prompt_redraw(input: &[u8]) -> Option<SemanticPromptRedraw> {
    match input {
        b"0" => Some(SemanticPromptRedraw::False),
        b"1" => Some(SemanticPromptRedraw::True),
        b"last" => Some(SemanticPromptRedraw::Last),
        _ => None,
    }
}

/// OSC 66 — Kitty text sizing protocol.
pub(super) fn parse_text_sizing(params: &[&[u8]]) -> Option<TextSizing> {
    if params.len() < 3 {
        return None;
    }

    let text = join_osc_text_params(&params[2..]);
    if text.len() > 4096 {
        return None;
    }

    let mut sizing = TextSizing::new(simd_utf8::from_utf8_lossy_fast(&text));
    if params[1].is_empty() {
        return Some(sizing);
    }

    for item in params[1].split(|b| *b == b':') {
        let Some(eq_idx) = item.iter().position(|b| *b == b'=') else {
            continue;
        };
        let key = &item[..eq_idx];
        let value = &item[eq_idx + 1..];

        match key {
            b"s" => sizing.scale = parse_bounded_u8(value, 1, 7)?,
            b"w" => {
                let width = parse_bounded_u8(value, 0, 7)?;
                sizing.width = (width != 0).then_some(width);
            }
            b"n" => {
                let numerator = parse_bounded_u8(value, 0, 15)?;
                let denominator = sizing.fractional_scale.map_or(0, |(_, d)| d);
                sizing.fractional_scale = Some((numerator, denominator));
            }
            b"d" => {
                let denominator = parse_bounded_u8(value, 0, 15)?;
                let numerator = sizing.fractional_scale.map_or(0, |(n, _)| n);
                sizing.fractional_scale = Some((numerator, denominator));
            }
            b"v" => sizing.vertical_align = parse_text_sizing_align(value)?,
            b"h" => sizing.horizontal_align = parse_text_sizing_align(value)?,
            _ => {}
        }
    }

    if let Some((numerator, denominator)) = sizing.fractional_scale {
        if denominator != 0 && denominator <= numerator {
            return None;
        }
    }

    Some(sizing)
}

fn join_osc_text_params(params: &[&[u8]]) -> Vec<u8> {
    let len =
        params.iter().map(|p| p.len()).sum::<usize>() + params.len().saturating_sub(1);
    let mut out = Vec::with_capacity(len);
    for (idx, param) in params.iter().enumerate() {
        if idx != 0 {
            out.push(b';');
        }
        out.extend_from_slice(param);
    }
    out
}

fn parse_bounded_u8(input: &[u8], min: u8, max: u8) -> Option<u8> {
    let value = parse_number(input)?;
    (min..=max).contains(&value).then_some(value)
}

fn parse_text_sizing_align(input: &[u8]) -> Option<TextSizingAlign> {
    match input {
        b"0" => Some(TextSizingAlign::Start),
        b"1" => Some(TextSizingAlign::End),
        b"2" => Some(TextSizingAlign::Center),
        _ => None,
    }
}

/// OSC 99 — Kitty desktop notifications.
pub(super) fn parse_kitty_notification(params: &[&[u8]]) -> Option<KittyNotification> {
    if params.len() < 3 {
        return None;
    }

    let mut id = None;
    let mut kind = KittyNotificationKind::Title;
    let mut done = true;
    let mut encoded = false;
    let mut close_report = false;
    let mut action_report = false;

    for item in params[1].split(|b| *b == b':') {
        if item.is_empty() {
            continue;
        }
        let Some(eq_idx) = item.iter().position(|b| *b == b'=') else {
            continue;
        };
        let key = &item[..eq_idx];
        let value = &item[eq_idx + 1..];

        match key {
            b"i" => id = parse_notification_id(value),
            b"p" => kind = parse_kitty_notification_kind(value)?,
            b"c" => close_report = parse_bool(value)?,
            b"a" => action_report = value == b"report",
            b"d" => done = parse_bool(value)?,
            b"e" => encoded = parse_bool(value)?,
            _ => {}
        }
    }

    let payload = join_osc_text_params(&params[2..]);
    if encoded {
        if payload.len() > 4096 {
            return None;
        }
        let bytes = crate::simd_base64::decode(&payload)?;
        Some(KittyNotification {
            id,
            kind,
            done,
            close_report,
            action_report,
            payload: simd_utf8::from_utf8_lossy_fast(&bytes),
        })
    } else {
        if payload.len() > 2048 {
            return None;
        }
        Some(KittyNotification {
            id,
            kind,
            done,
            close_report,
            action_report,
            payload: simd_utf8::from_utf8_lossy_fast(&payload),
        })
    }
}

fn parse_notification_id(input: &[u8]) -> Option<String> {
    (!input.is_empty()
        && input
            .iter()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'+' | b'.')))
    .then(|| simd_utf8::from_utf8_lossy_fast(input))
}

fn parse_kitty_notification_kind(input: &[u8]) -> Option<KittyNotificationKind> {
    match input {
        b"title" => Some(KittyNotificationKind::Title),
        b"body" => Some(KittyNotificationKind::Body),
        b"close" => Some(KittyNotificationKind::Close),
        b"alive" => Some(KittyNotificationKind::Alive),
        b"?" => Some(KittyNotificationKind::Query),
        b"buttons" => Some(KittyNotificationKind::Buttons),
        b"icon" => Some(KittyNotificationKind::Icon),
        _ => None,
    }
}

/// OSC 10/11/12: dynamic color set/query, applied to consecutive named
/// colors starting at `dynamic_code - 10`.
pub(super) fn parse_dynamic_colors(params: &[&[u8]]) -> Option<Vec<DynamicColorEntry>> {
    if params.len() < 2 {
        return None;
    }
    let base_code = parse_number(params[0])? as u16;
    let mut out = Vec::with_capacity(params.len() - 1);
    for (dynamic_code, param) in (base_code..).zip(params[1..].iter()) {
        // 10 is the first dynamic color (foreground).
        let offset = (dynamic_code as usize).checked_sub(10)?;
        let index_usize = NamedColor::Foreground as usize + offset;
        if index_usize > NamedColor::Cursor as usize {
            return None;
        }
        let index = match offset {
            0 => NamedColor::Foreground,
            1 => NamedColor::Background,
            2 => NamedColor::Cursor,
            _ => return None,
        };
        let spec = if *param == b"?" {
            ColorSpec::Query
        } else if let Some(c) = xparse_color(param) {
            ColorSpec::Set(c)
        } else {
            return None;
        };
        out.push(DynamicColorEntry {
            index,
            dynamic_code,
            spec,
        });
    }
    Some(out)
}

/// OSC 22: Kitty mouse pointer shape operation.
pub(super) fn parse_mouse_cursor_operation(param: &[u8]) -> Option<MouseCursorOp> {
    let value = simd_utf8::from_utf8_fast(param).ok()?.trim();
    if value.is_empty() {
        return Some(MouseCursorOp::Set(None));
    }

    let mut chars = value.chars();
    let first = chars.next()?;
    let rest = chars.as_str().trim();

    match first {
        '=' => Some(MouseCursorOp::Set(Some(parse_mouse_cursor_icon(rest)?))),
        '>' => {
            let icons = parse_mouse_cursor_icon_list(rest);
            (!icons.is_empty()).then_some(MouseCursorOp::Push(icons))
        }
        '<' => Some(MouseCursorOp::Pop),
        '?' => Some(MouseCursorOp::Query(parse_mouse_cursor_query_list(rest))),
        _ => Some(MouseCursorOp::Set(Some(parse_mouse_cursor_icon(value)?))),
    }
}

fn parse_mouse_cursor_icon(shape: &str) -> Option<CursorIcon> {
    CursorIcon::from_str(shape.trim()).ok()
}

fn parse_mouse_cursor_icon_list(value: &str) -> Vec<CursorIcon> {
    value
        .split(',')
        .filter_map(parse_mouse_cursor_icon)
        .collect()
}

fn parse_mouse_cursor_query_list(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|shape| !shape.is_empty())
        .map(str::to_owned)
        .collect()
}

/// OSC 50: `CursorShape=N` text cursor selector.
pub(super) fn parse_cursor_shape(params: &[&[u8]]) -> Option<CursorShape> {
    if params.len() < 2 || params[1].len() < 13 || params[1][0..12] != *b"CursorShape=" {
        return None;
    }
    match params[1][12] as char {
        '0' => Some(CursorShape::Block),
        '1' => Some(CursorShape::Beam),
        '2' => Some(CursorShape::Underline),
        _ => None,
    }
}

/// OSC 52: clipboard load (`?`) or store (base64 payload).
pub(super) fn parse_clipboard<'a>(params: &[&'a [u8]]) -> Option<ClipboardOp<'a>> {
    if params.len() < 3 {
        return None;
    }
    let kind = *params[1].first().unwrap_or(&b'c');
    Some(if params[2] == b"?" {
        ClipboardOp::Load { kind }
    } else {
        ClipboardOp::Store {
            kind,
            payload: params[2],
        }
    })
}

/// OSC 5522: Kitty rich clipboard metadata/payload parser.
pub(super) fn parse_kitty_clipboard<'a>(
    params: &[&'a [u8]],
) -> Option<KittyClipboard<'a>> {
    let metadata = *params.get(1)?;
    let operation = match metadata_value(metadata, b"type")? {
        b"read" => KittyClipboardOperation::Read,
        b"write" => KittyClipboardOperation::Write,
        b"wdata" => KittyClipboardOperation::Wdata,
        b"walias" => KittyClipboardOperation::Walias,
        _ => return None,
    };
    let location = match metadata_value(metadata, b"loc") {
        None => KittyClipboardLocation::Clipboard,
        Some(b"primary") => KittyClipboardLocation::Selection,
        Some(_) => KittyClipboardLocation::Unsupported,
    };

    Some(KittyClipboard {
        operation,
        location,
        mime: metadata_value(metadata, b"mime"),
        payload: params.get(2).copied(),
    })
}

/// OSC 5113: Kitty file transfer command parser.
pub(super) fn parse_kitty_file_transfer(params: &[&[u8]]) -> Option<KittyFileTransfer> {
    let mut action = None;
    let mut id = None;
    let mut file_id = None;
    let mut quiet = 0;

    for param in params.get(1..)? {
        let eq = param.iter().position(|b| *b == b'=')?;
        let key = trim_ascii(&param[..eq]);
        let value = trim_ascii(&param[eq + 1..]);
        if !is_kitty_file_transfer_key(key) {
            return None;
        }

        match key {
            b"ac" | b"action" => action = Some(parse_kitty_file_transfer_action(value)?),
            b"id" => id = Some(parse_kitty_file_transfer_safe_string(value)?),
            b"fid" | b"file_id" => {
                file_id = Some(parse_kitty_file_transfer_safe_string(value)?)
            }
            b"q" | b"quiet" => {
                quiet = parse_number(value)?;
                if quiet > 2 {
                    return None;
                }
            }
            _ => {}
        }
    }

    Some(KittyFileTransfer {
        action: action?,
        id: id?,
        file_id,
        quiet,
    })
}

fn parse_kitty_file_transfer_action(input: &[u8]) -> Option<KittyFileTransferAction> {
    match input {
        b"send" => Some(KittyFileTransferAction::Send),
        b"file" => Some(KittyFileTransferAction::File),
        b"data" => Some(KittyFileTransferAction::Data),
        b"end_data" => Some(KittyFileTransferAction::EndData),
        b"receive" => Some(KittyFileTransferAction::Receive),
        b"cancel" => Some(KittyFileTransferAction::Cancel),
        b"status" => Some(KittyFileTransferAction::Status),
        b"finish" | b"finished" => Some(KittyFileTransferAction::Finish),
        _ => None,
    }
}

fn parse_kitty_file_transfer_safe_string(input: &[u8]) -> Option<String> {
    if input.is_empty() || !input.iter().all(is_kitty_file_transfer_safe_byte) {
        return None;
    }
    Some(simd_utf8::from_utf8_fast(input).ok()?.to_owned())
}

fn is_kitty_file_transfer_key(input: &[u8]) -> bool {
    !input.is_empty()
        && input
            .iter()
            .all(|b| b.is_ascii_alphanumeric() || *b == b'_')
}

fn is_kitty_file_transfer_safe_byte(byte: &u8) -> bool {
    byte.is_ascii_alphanumeric()
        || matches!(*byte, b'_' | b':' | b'.' | b'/' | b'@' | b'-')
}

fn metadata_value<'a>(metadata: &'a [u8], key: &[u8]) -> Option<&'a [u8]> {
    for item in metadata.split(|b| *b == b':') {
        let item = trim_ascii(item);
        let Some(eq) = item.iter().position(|b| *b == b'=') else {
            continue;
        };
        let item_key = trim_ascii(&item[..eq]);
        if item_key == key {
            return Some(trim_ascii(&item[eq + 1..]));
        }
    }
    None
}

fn trim_ascii(bytes: &[u8]) -> &[u8] {
    let mut start = 0;
    let mut end = bytes.len();
    while start < end && bytes[start].is_ascii_whitespace() {
        start += 1;
    }
    while end > start && bytes[end - 1].is_ascii_whitespace() {
        end -= 1;
    }
    &bytes[start..end]
}

/// OSC 104: reset palette colors. Empty/omitted parameter list means "all".
pub(super) fn parse_palette_reset(params: &[&[u8]]) -> PaletteReset {
    if params.len() == 1 || params[1].is_empty() {
        return PaletteReset::All;
    }
    let indices = params[1..].iter().filter_map(|p| parse_number(p)).collect();
    PaletteReset::Indices(indices)
}

#[cfg(test)]
mod tests {
    // Test lane: default
    use super::*;

    #[test]
    // Defends: Kitty OSC 21 accepts Ghostty-compatible color syntaxes beyond legacy hex.
    fn kitty_color_parses_rgbi_and_named_colors() {
        assert_eq!(
            xparse_color(b"rgbi:1.0/0.5/0.0"),
            Some(ColorRgb {
                r: 255,
                g: 128,
                b: 0,
            })
        );
        assert_eq!(
            xparse_color(b"aliceblue"),
            Some(ColorRgb {
                r: 0xf0,
                g: 0xf8,
                b: 0xff,
            })
        );
    }

    #[test]
    // Defends: Kitty OSC 21 preserves supported keyed colors and unknown query keys without failing the packet.
    fn kitty_color_entries_parse_set_query_reset_and_unknown_query() {
        let entries = parse_kitty_color_entries(&[
            b"21".as_slice(),
            b"foreground=?".as_slice(),
            b"background=rgb:f0/f8/ff".as_slice(),
            b"cursor=aliceblue".as_slice(),
            b"cursor_text".as_slice(),
            b"visual_bell=".as_slice(),
            b"transparent_background=?".as_slice(),
            b"selection_foreground=#xxxyyzz".as_slice(),
            b"selection_background=?".as_slice(),
            b"2=?".as_slice(),
            b"3=rgbi:1.0/1.0/1.0".as_slice(),
        ])
        .unwrap();

        assert_eq!(entries.len(), 9);
        assert!(matches!(entries[0].spec, KittyColorSpec::Query));
        assert_eq!(entries[0].index, Some(NamedColor::Foreground as usize));
        assert!(matches!(
            entries[1].spec,
            KittyColorSpec::Set(ColorRgb {
                r: 0xf0,
                g: 0xf8,
                b: 0xff
            })
        ));
        assert_eq!(entries[1].index, Some(NamedColor::Background as usize));
        assert!(matches!(
            entries[2].spec,
            KittyColorSpec::Set(ColorRgb {
                r: 0xf0,
                g: 0xf8,
                b: 0xff
            })
        ));
        assert_eq!(entries[2].index, Some(NamedColor::Cursor as usize));
        assert!(matches!(entries[3].spec, KittyColorSpec::Reset));
        assert_eq!(entries[3].key, "cursor_text");
        assert_eq!(entries[3].index, Some(NamedColor::CursorText as usize));
        assert!(matches!(entries[4].spec, KittyColorSpec::Reset));
        assert_eq!(entries[4].key, "visual_bell");
        assert_eq!(entries[4].index, Some(NamedColor::VisualBell as usize));
        assert!(matches!(entries[5].spec, KittyColorSpec::Query));
        assert_eq!(
            entries[5].index,
            Some(NamedColor::TransparentBackground as usize)
        );
        assert!(matches!(entries[6].spec, KittyColorSpec::Query));
        assert_eq!(entries[6].key, "selection_background");
        assert_eq!(
            entries[6].index,
            Some(NamedColor::SelectionBackground as usize)
        );
        assert!(matches!(entries[7].spec, KittyColorSpec::Query));
        assert_eq!(entries[7].index, Some(2));
        assert!(matches!(
            entries[8].spec,
            KittyColorSpec::Set(ColorRgb {
                r: 255,
                g: 255,
                b: 255
            })
        ));
        assert_eq!(entries[8].index, Some(3));
    }

    #[test]
    // Defends: Ghostty/Kitty's second transparent background key maps to Rio's transparent-background slot.
    fn kitty_color_parses_second_transparent_background_key() {
        let entries = parse_kitty_color_entries(&[
            b"21".as_slice(),
            b"second_transparent_background=?".as_slice(),
        ])
        .unwrap();

        assert_eq!(entries.len(), 1);
        assert!(matches!(entries[0].spec, KittyColorSpec::Query));
        assert_eq!(
            entries[0].index,
            Some(NamedColor::TransparentBackground as usize)
        );
    }

    #[test]
    // Defends: Kitty OSC 22 set/reset/push/pop/query prefixes are parsed as distinct operations.
    fn mouse_cursor_operations_parse_kitty_pointer_shapes() {
        assert!(matches!(
            parse_mouse_cursor_operation(b"pointer"),
            Some(MouseCursorOp::Set(Some(CursorIcon::Pointer)))
        ));
        assert!(matches!(
            parse_mouse_cursor_operation(b"=crosshair"),
            Some(MouseCursorOp::Set(Some(CursorIcon::Crosshair)))
        ));
        assert!(matches!(
            parse_mouse_cursor_operation(b""),
            Some(MouseCursorOp::Set(None))
        ));
        assert!(matches!(
            parse_mouse_cursor_operation(b">wait,pointer"),
            Some(MouseCursorOp::Push(icons))
                if icons == vec![CursorIcon::Wait, CursorIcon::Pointer]
        ));
        assert!(matches!(
            parse_mouse_cursor_operation(b"<ignored"),
            Some(MouseCursorOp::Pop)
        ));
        assert!(matches!(
            parse_mouse_cursor_operation(b"?pointer,crosshair,no-such-name"),
            Some(MouseCursorOp::Query(queries))
                if queries == vec!["pointer", "crosshair", "no-such-name"]
        ));
    }

    #[test]
    // Defends: OSC 133 shell prompt metadata is parsed without leaking into the unhandled OSC path.
    fn semantic_prompt_parses_prompt_options() {
        let prompt = parse_semantic_prompt(&[
            b"133".as_slice(),
            b"A".as_slice(),
            b"aid=abc".as_slice(),
            b"cl=line".as_slice(),
            b"redraw=last".as_slice(),
            b"click_events=1".as_slice(),
        ])
        .unwrap();

        assert_eq!(prompt.action, SemanticPromptAction::FreshLineNewPrompt);
        assert_eq!(prompt.aid.as_deref(), Some("abc"));
        assert_eq!(prompt.click, Some(SemanticPromptClick::Line));
        assert_eq!(prompt.redraw, Some(SemanticPromptRedraw::Last));
        assert_eq!(prompt.click_events, Some(true));
    }

    #[test]
    // Defends: command-finish markers keep their optional exit code, matching Ghostty shell integration.
    fn semantic_prompt_parses_end_command_exit_code() {
        let prompt = parse_semantic_prompt(&[
            b"133".as_slice(),
            b"D".as_slice(),
            b"12".as_slice(),
        ])
        .unwrap();

        assert_eq!(prompt.action, SemanticPromptAction::EndCommand);
        assert_eq!(prompt.exit_code, Some(12));
    }

    #[test]
    // Defends: malformed or unknown OSC 133 actions remain unsupported instead of creating bogus state.
    fn semantic_prompt_rejects_unknown_action() {
        assert!(parse_semantic_prompt(&[b"133".as_slice(), b"Z".as_slice()]).is_none());
    }

    #[test]
    // Defends: OSC 66 metadata is colon-separated and preserves semicolons in text payloads.
    fn text_sizing_parses_metadata_and_text() {
        let sizing = parse_text_sizing(&[
            b"66".as_slice(),
            b"s=2:w=3:n=1:d=2:v=2:h=1".as_slice(),
            b"a".as_slice(),
            b"b".as_slice(),
        ])
        .unwrap();

        assert_eq!(sizing.scale, 2);
        assert_eq!(sizing.width, Some(3));
        assert_eq!(sizing.fractional_scale, Some((1, 2)));
        assert_eq!(sizing.vertical_align, TextSizingAlign::Center);
        assert_eq!(sizing.horizontal_align, TextSizingAlign::End);
        assert_eq!(sizing.text, "a;b");
    }

    #[test]
    // Defends: invalid fractional scale metadata is rejected instead of creating undefined render state.
    fn text_sizing_rejects_invalid_fractional_scale() {
        assert!(parse_text_sizing(&[
            b"66".as_slice(),
            b"n=2:d=1".as_slice(),
            b"x".as_slice(),
        ])
        .is_none());
    }

    #[test]
    // Defends: OSC 5113 file-transfer commands parse only safe echoed identifiers and bounded quiet values.
    fn kitty_file_transfer_parses_safe_command_metadata() {
        let transfer = parse_kitty_file_transfer(&[
            b"5113".as_slice(),
            b"ac=send".as_slice(),
            b"id=session-1".as_slice(),
            b"fid=file_1".as_slice(),
            b"q=2".as_slice(),
            b"unknown=ignored".as_slice(),
        ])
        .unwrap();

        assert_eq!(transfer.action, KittyFileTransferAction::Send);
        assert_eq!(transfer.id, "session-1");
        assert_eq!(transfer.file_id.as_deref(), Some("file_1"));
        assert_eq!(transfer.quiet, 2);
        assert!(parse_kitty_file_transfer(&[
            b"5113".as_slice(),
            b"ac=send".as_slice(),
            b"id=bad;id".as_slice(),
        ])
        .is_none());
        assert!(parse_kitty_file_transfer(&[
            b"5113".as_slice(),
            b"ac=send".as_slice(),
            b"id=session-1".as_slice(),
            b"q=3".as_slice(),
        ])
        .is_none());
    }

    #[test]
    // Defends: Kitty OSC 99 notification metadata is parsed without relying on legacy OSC 9/777 forms.
    fn kitty_notification_parses_title_chunk() {
        let notification = parse_kitty_notification(&[
            b"99".as_slice(),
            b"i=build:d=0:p=title".as_slice(),
            b"Build".as_slice(),
        ])
        .unwrap();

        assert_eq!(notification.id.as_deref(), Some("build"));
        assert_eq!(notification.kind, KittyNotificationKind::Title);
        assert!(!notification.done);
        assert!(!notification.close_report);
        assert!(!notification.action_report);
        assert_eq!(notification.payload, "Build");
    }

    #[test]
    // Defends: OSC 99 IDs are sanitized before they can be echoed back in protocol replies.
    fn kitty_notification_parses_close_report_and_rejects_bad_ids() {
        let notification = parse_kitty_notification(&[
            b"99".as_slice(),
            b"i=bad;id:c=1:p=alive".as_slice(),
            b"".as_slice(),
        ])
        .unwrap();

        assert_eq!(notification.id, None);
        assert!(notification.close_report);
        assert!(!notification.action_report);
        assert_eq!(notification.kind, KittyNotificationKind::Alive);
    }

    #[test]
    // Defends: OSC 99 a=report is parsed so activation and button callbacks can be routed.
    fn kitty_notification_parses_action_report() {
        let notification = parse_kitty_notification(&[
            b"99".as_slice(),
            b"i=job:a=report:p=title".as_slice(),
            b"Done".as_slice(),
        ])
        .unwrap();

        assert_eq!(notification.id.as_deref(), Some("job"));
        assert!(notification.action_report);
    }

    #[test]
    // Defends: OSC 99 base64 payloads decode to plain UTF-8 before notification display.
    fn kitty_notification_decodes_base64_payload() {
        let notification = parse_kitty_notification(&[
            b"99".as_slice(),
            b"p=body:e=1".as_slice(),
            b"aGVsbG8=".as_slice(),
        ])
        .unwrap();

        assert_eq!(notification.kind, KittyNotificationKind::Body);
        assert!(!notification.action_report);
        assert_eq!(notification.payload, "hello");
    }
}
