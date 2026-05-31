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
    SemanticPrompt, SemanticPromptAction, SemanticPromptClick, SemanticPromptKind,
    SemanticPromptRedraw, TextSizing, TextSizingAlign,
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

/// Parse an `xterm`-style color value (`#rgb`, `#rrggbb`, `rgb:r/g/b`).
pub(super) fn xparse_color(color: &[u8]) -> Option<ColorRgb> {
    if !color.is_empty() && color[0] == b'#' {
        parse_legacy_color(&color[1..])
    } else if color.len() >= 4 && &color[..4] == b"rgb:" {
        parse_rgb_color(&color[4..])
    } else {
        None
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

/// OSC 22: mouse cursor icon name.
pub(super) fn parse_mouse_cursor_icon(param: &[u8]) -> Option<CursorIcon> {
    let shape = simd_utf8::from_utf8_lossy_fast(param);
    CursorIcon::from_str(&shape).ok()
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
    use super::*;

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
}
