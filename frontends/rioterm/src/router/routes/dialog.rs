use rio_backend::sugarloaf::text::DrawOpts;
use rio_backend::sugarloaf::Sugarloaf;

const SIDE_MARGIN: f32 = 28.0;
const MODAL_MIN_W: f32 = 340.0;
const MODAL_MAX_W: f32 = 480.0;
const MODAL_H: f32 = 180.0;
const MODAL_MIN_H: f32 = 132.0;
const CONTENT_PAD_X: f32 = 34.0;
const ACTION_GAP: f32 = 56.0;
const BUTTON_H: f32 = 38.0;
const BUTTON_MAX_W: f32 = 170.0;
const HEADING_SIZE: f32 = 20.0;
const ACTION_SIZE: f32 = 14.0;
const MIN_TEXT_ACTION_GAP: f32 = 10.0;
const BUTTON_BG: [f32; 4] = [0.145, 0.160, 0.180, 0.98];

#[derive(Debug)]
struct DialogLayout {
    modal_x: f32,
    modal_y: f32,
    modal_w: f32,
    modal_h: f32,
    button_w: f32,
    action_gap: f32,
    actions_x: f32,
    button_y: f32,
    heading_y: f32,
}

fn compute_layout(win_w: f32, win_h: f32) -> DialogLayout {
    let available_w = (win_w - SIDE_MARGIN * 2.0)
        .max(win_w * 0.86)
        .min(win_w)
        .max(0.0);
    let modal_w = (win_w * 0.42)
        .clamp(MODAL_MIN_W, MODAL_MAX_W)
        .min(available_w)
        .max(0.0);
    let modal_h = MODAL_H
        .min((win_h - 32.0).max(MODAL_MIN_H))
        .min(win_h)
        .max(0.0);
    let modal_x = (win_w - modal_w) / 2.0;
    let modal_y = (win_h - modal_h) / 2.0;

    let content_pad_x = CONTENT_PAD_X.min((modal_w * 0.12).max(12.0));
    let button_w =
        ((modal_w - content_pad_x * 2.0 - ACTION_GAP) / 2.0).clamp(0.0, BUTTON_MAX_W);
    let actions_w = button_w * 2.0 + ACTION_GAP;
    let actions_x = modal_x + (modal_w - actions_w) / 2.0;

    let bottom_pad = if modal_h < MODAL_H { 20.0 } else { 34.0 };
    let button_y = modal_y + modal_h - BUTTON_H - bottom_pad;
    let desired_heading_y = modal_y + if modal_h < MODAL_H { 34.0 } else { 54.0 };
    let heading_y = desired_heading_y.min(button_y - HEADING_SIZE - MIN_TEXT_ACTION_GAP);

    DialogLayout {
        modal_x,
        modal_y,
        modal_w,
        modal_h,
        button_w,
        action_gap: ACTION_GAP,
        actions_x,
        button_y,
        heading_y,
    }
}

#[inline]
pub fn screen(
    sugarloaf: &mut Sugarloaf,
    dimensions: (f32, f32, f32),
    heading_content: &str,
    confirm_content: &str,
    quit_content: &str,
) {
    let (width, height, scale) = dimensions;
    let win_w = width / scale;
    let win_h = height / scale;

    let layout = compute_layout(win_w, win_h);

    let heading_opts = DrawOpts {
        font_size: HEADING_SIZE,
        color: [245, 247, 250, 255],
        bold: true,
        ..DrawOpts::default()
    };
    let action_opts = DrawOpts {
        font_size: ACTION_SIZE,
        color: [235, 240, 246, 255],
        bold: true,
        ..DrawOpts::default()
    };

    let (heading_w, confirm_w, quit_w) = {
        let ui = sugarloaf.text_mut();
        (
            ui.measure(heading_content, &heading_opts),
            ui.measure(confirm_content, &action_opts),
            ui.measure(quit_content, &action_opts),
        )
    };

    sugarloaf.rect(None, 0.0, 0.0, win_w, win_h, [0.0, 0.0, 0.0, 0.24], 0.0, 18);

    sugarloaf.rounded_rect(
        None,
        layout.modal_x,
        layout.modal_y + 9.0,
        layout.modal_w,
        layout.modal_h,
        [0.0, 0.0, 0.0, 0.36],
        0.02,
        22.0,
        19,
    );
    sugarloaf.rounded_rect(
        None,
        layout.modal_x - 1.0,
        layout.modal_y - 1.0,
        layout.modal_w + 2.0,
        layout.modal_h + 2.0,
        [0.95, 0.36, 0.14, 0.72],
        0.03,
        22.0,
        20,
    );
    sugarloaf.rounded_rect(
        None,
        layout.modal_x,
        layout.modal_y,
        layout.modal_w,
        layout.modal_h,
        [0.055, 0.060, 0.070, 0.98],
        0.04,
        21.0,
        21,
    );
    sugarloaf.rounded_rect(
        None,
        layout.actions_x,
        layout.button_y,
        layout.button_w,
        BUTTON_H,
        BUTTON_BG,
        0.05,
        10.0,
        22,
    );
    sugarloaf.rounded_rect(
        None,
        layout.actions_x + layout.button_w + layout.action_gap,
        layout.button_y,
        layout.button_w,
        BUTTON_H,
        BUTTON_BG,
        0.05,
        10.0,
        22,
    );

    let heading_x = layout.modal_x + (layout.modal_w - heading_w) / 2.0;
    let confirm_x = layout.actions_x + (layout.button_w - confirm_w) / 2.0;
    let quit_x = layout.actions_x
        + layout.button_w
        + layout.action_gap
        + (layout.button_w - quit_w) / 2.0;
    let action_y = layout.button_y + (BUTTON_H - ACTION_SIZE) / 2.0 - 1.0;

    let ui = sugarloaf.text_mut();
    ui.draw(heading_x, layout.heading_y, heading_content, &heading_opts);
    ui.draw(confirm_x, action_y, confirm_content, &action_opts);
    ui.draw(quit_x, action_y, quit_content, &action_opts);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compact_layout_keeps_title_above_actions() {
        for (win_w, win_h) in [(320.0, 160.0), (260.0, 140.0), (220.0, 120.0)] {
            let layout = compute_layout(win_w, win_h);

            assert!(
                layout.heading_y + HEADING_SIZE + MIN_TEXT_ACTION_GAP <= layout.button_y,
                "title/action overlap at {win_w}x{win_h}: {layout:?}"
            );
        }
    }

    #[test]
    fn compact_layout_keeps_actions_inside_modal() {
        for (win_w, win_h) in [(320.0, 160.0), (260.0, 140.0), (220.0, 120.0)] {
            let layout = compute_layout(win_w, win_h);
            let actions_w = layout.button_w * 2.0 + layout.action_gap;

            assert!(
                actions_w <= layout.modal_w,
                "actions overflow at {win_w}x{win_h}: {layout:?}"
            );
        }
    }

    #[test]
    fn standard_layout_is_taller_narrower_and_spreads_actions() {
        let layout = compute_layout(1920.0, 1080.0);

        assert_eq!(layout.modal_w, MODAL_MAX_W);
        assert_eq!(layout.modal_h, MODAL_H);
        assert!(
            layout.action_gap >= 48.0,
            "actions should have visible separation: {layout:?}"
        );
        assert!(
            layout.button_w >= 160.0,
            "standard actions should be wider: {layout:?}"
        );
    }
}
