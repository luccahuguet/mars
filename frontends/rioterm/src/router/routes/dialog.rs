use crate::layout::ContextDimension;
use rio_backend::sugarloaf::text::DrawOpts;
use rio_backend::sugarloaf::Sugarloaf;

#[inline]
pub fn screen(
    sugarloaf: &mut Sugarloaf,
    context_dimension: &ContextDimension,
    heading_content: &str,
    confirm_content: &str,
    quit_content: &str,
) {
    let layout = sugarloaf.window_size();
    let scale = context_dimension.dimension.scale;
    let win_w = layout.width / scale;
    let win_h = layout.height / scale;

    const SIDE_MARGIN: f32 = 28.0;
    const MODAL_MIN_W: f32 = 380.0;
    const MODAL_MAX_W: f32 = 540.0;
    const MODAL_H: f32 = 154.0;
    const CONTENT_PAD_X: f32 = 34.0;
    const ACTION_GAP: f32 = 12.0;
    const BUTTON_H: f32 = 38.0;
    const BUTTON_MAX_W: f32 = 146.0;
    const HEADING_SIZE: f32 = 20.0;
    const ACTION_SIZE: f32 = 14.0;

    let available_w = (win_w - SIDE_MARGIN * 2.0).max(win_w * 0.86);
    let modal_w = (win_w * 0.42)
        .clamp(MODAL_MIN_W, MODAL_MAX_W)
        .min(available_w);
    let modal_h = MODAL_H.min((win_h - 32.0).max(118.0));
    let modal_x = (win_w - modal_w) / 2.0;
    let modal_y = (win_h - modal_h) / 2.0;

    let heading_opts = DrawOpts {
        font_size: HEADING_SIZE,
        color: [245, 247, 250, 255],
        bold: true,
        ..DrawOpts::default()
    };
    let confirm_opts = DrawOpts {
        font_size: ACTION_SIZE,
        color: [255, 255, 255, 255],
        bold: true,
        ..DrawOpts::default()
    };
    let quit_opts = DrawOpts {
        font_size: ACTION_SIZE,
        color: [220, 225, 232, 255],
        ..DrawOpts::default()
    };

    let (heading_w, confirm_w, quit_w) = {
        let ui = sugarloaf.text_mut();
        (
            ui.measure(heading_content, &heading_opts),
            ui.measure(confirm_content, &confirm_opts),
            ui.measure(quit_content, &quit_opts),
        )
    };

    let button_w =
        ((modal_w - CONTENT_PAD_X * 2.0 - ACTION_GAP) / 2.0).clamp(104.0, BUTTON_MAX_W);
    let actions_w = button_w * 2.0 + ACTION_GAP;
    let actions_x = modal_x + (modal_w - actions_w) / 2.0;
    let button_y = modal_y + modal_h - BUTTON_H - 28.0;

    sugarloaf.rect(None, 0.0, 0.0, win_w, win_h, [0.0, 0.0, 0.0, 0.24], 0.0, 18);

    sugarloaf.rounded_rect(
        None,
        modal_x,
        modal_y + 9.0,
        modal_w,
        modal_h,
        [0.0, 0.0, 0.0, 0.36],
        0.02,
        22.0,
        19,
    );
    sugarloaf.rounded_rect(
        None,
        modal_x - 1.0,
        modal_y - 1.0,
        modal_w + 2.0,
        modal_h + 2.0,
        [0.95, 0.36, 0.14, 0.72],
        0.03,
        22.0,
        20,
    );
    sugarloaf.rounded_rect(
        None,
        modal_x,
        modal_y,
        modal_w,
        modal_h,
        [0.055, 0.060, 0.070, 0.98],
        0.04,
        21.0,
        21,
    );
    sugarloaf.rounded_rect(
        None,
        modal_x + (modal_w - 64.0) / 2.0,
        modal_y + 18.0,
        64.0,
        3.0,
        [0.95, 0.36, 0.14, 0.92],
        0.05,
        2.0,
        22,
    );

    sugarloaf.rounded_rect(
        None,
        actions_x,
        button_y,
        button_w,
        BUTTON_H,
        [0.95, 0.36, 0.14, 0.98],
        0.05,
        10.0,
        22,
    );
    sugarloaf.rounded_rect(
        None,
        actions_x + button_w + ACTION_GAP,
        button_y,
        button_w,
        BUTTON_H,
        [0.145, 0.160, 0.180, 0.98],
        0.05,
        10.0,
        22,
    );

    let heading_x = modal_x + (modal_w - heading_w) / 2.0;
    let heading_y = modal_y + 47.0;
    let confirm_x = actions_x + (button_w - confirm_w) / 2.0;
    let quit_x = actions_x + button_w + ACTION_GAP + (button_w - quit_w) / 2.0;
    let action_y = button_y + (BUTTON_H - ACTION_SIZE) / 2.0 - 1.0;

    let ui = sugarloaf.text_mut();
    ui.draw(heading_x, heading_y, heading_content, &heading_opts);
    ui.draw(confirm_x, action_y, confirm_content, &confirm_opts);
    ui.draw(quit_x, action_y, quit_content, &quit_opts);
}
