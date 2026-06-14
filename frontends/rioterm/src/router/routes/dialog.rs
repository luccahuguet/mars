use rio_backend::sugarloaf::text::{DrawOpts, Text};
use rio_backend::sugarloaf::Sugarloaf;

const CARD_WIDTH: f32 = 420.0;
const CARD_HEIGHT: f32 = 198.0;
const CARD_MIN_WIDTH: f32 = 280.0;
const WINDOW_MARGIN: f32 = 24.0;
const CARD_RADIUS: f32 = 12.0;
const CARD_PADDING_X: f32 = 24.0;

const TITLE_TEXT: &str = "Close Window?";
const BODY_LINE_1: &str = "All terminal sessions in this window will be";
const BODY_LINE_2: &str = "terminated.";
const CANCEL_TEXT: &str = "Cancel";
const CLOSE_TEXT: &str = "Close";

const TITLE_FONT_SIZE: f32 = 18.0;
const BODY_FONT_SIZE: f32 = 13.0;
const BUTTON_FONT_SIZE: f32 = 14.0;

const TITLE_Y_OFFSET: f32 = 32.0;
const BODY_Y_OFFSET: f32 = 76.0;
const BODY_LINE_HEIGHT: f32 = 19.0;
const BUTTON_HEIGHT: f32 = 42.0;
const BUTTON_GAP: f32 = 12.0;
const BUTTON_BOTTOM_OFFSET: f32 = 24.0;
const BUTTON_RADIUS: f32 = 8.0;
const BUTTON_STROKE: f32 = 1.5;

const BACKDROP_COLOR: [f32; 4] = [0.0, 0.0, 0.0, 0.55];
const SHADOW_COLOR: [f32; 4] = [0.0, 0.0, 0.0, 0.28];
const CARD_COLOR: [f32; 4] = [0.015, 0.015, 0.018, 0.98];
const TITLE_COLOR: [u8; 4] = [225, 225, 226, 255];
const BODY_COLOR: [u8; 4] = [177, 173, 176, 255];
const CANCEL_BORDER_COLOR: [f32; 4] = [1.0, 0.34, 0.18, 1.0];
const CANCEL_FILL_COLOR: [f32; 4] = [0.06, 0.065, 0.07, 1.0];
const CANCEL_HOVER_FILL_COLOR: [f32; 4] = [0.10, 0.10, 0.105, 1.0];
const CLOSE_FILL_COLOR: [f32; 4] = [0.20, 0.105, 0.105, 1.0];
const CLOSE_HOVER_FILL_COLOR: [f32; 4] = [0.28, 0.125, 0.12, 1.0];
const CANCEL_TEXT_COLOR: [u8; 4] = [205, 202, 204, 255];
const CLOSE_TEXT_COLOR: [u8; 4] = [255, 191, 186, 255];

const DEPTH_BACKDROP: f32 = 0.0;
const DEPTH_SHADOW: f32 = 0.05;
const DEPTH_CARD: f32 = 0.1;
const DEPTH_BUTTON: f32 = 0.2;
const ORDER: u8 = 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfirmQuitAction {
    Cancel,
    Close,
}

#[derive(Debug, Clone, Copy)]
struct Rect {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
}

impl Rect {
    fn contains(self, x: f32, y: f32) -> bool {
        x >= self.x
            && x <= self.x + self.width
            && y >= self.y
            && y <= self.y + self.height
    }
}

#[derive(Debug, Clone, Copy)]
struct ConfirmQuitLayout {
    card: Rect,
    cancel_button: Rect,
    close_button: Rect,
}

pub fn action_at(
    sugarloaf: &Sugarloaf,
    mouse_x: f64,
    mouse_y: f64,
) -> Option<ConfirmQuitAction> {
    let layout = logical_layout(sugarloaf);
    let scale = sugarloaf.scale_factor();
    let x = mouse_x as f32 / scale;
    let y = mouse_y as f32 / scale;

    if layout.cancel_button.contains(x, y) {
        Some(ConfirmQuitAction::Cancel)
    } else if layout.close_button.contains(x, y) {
        Some(ConfirmQuitAction::Close)
    } else {
        None
    }
}

#[inline]
pub fn screen(sugarloaf: &mut Sugarloaf, hovered_action: Option<ConfirmQuitAction>) {
    let window = sugarloaf.window_size();
    let scale = sugarloaf.scale_factor();
    let win_w = window.width / scale;
    let win_h = window.height / scale;
    let layout = layout_for_size(win_w, win_h);

    sugarloaf.rect(
        None,
        0.0,
        0.0,
        win_w,
        win_h,
        BACKDROP_COLOR,
        DEPTH_BACKDROP,
        ORDER,
    );

    sugarloaf.rounded_rect(
        None,
        layout.card.x,
        layout.card.y + 3.0,
        layout.card.width,
        layout.card.height,
        SHADOW_COLOR,
        DEPTH_SHADOW,
        CARD_RADIUS,
        ORDER,
    );
    sugarloaf.rounded_rect(
        None,
        layout.card.x,
        layout.card.y,
        layout.card.width,
        layout.card.height,
        CARD_COLOR,
        DEPTH_CARD,
        CARD_RADIUS,
        ORDER,
    );

    let cancel_fill = if hovered_action == Some(ConfirmQuitAction::Cancel) {
        CANCEL_HOVER_FILL_COLOR
    } else {
        CANCEL_FILL_COLOR
    };
    stroked_button(
        sugarloaf,
        layout.cancel_button,
        CANCEL_BORDER_COLOR,
        cancel_fill,
    );

    let close_fill = if hovered_action == Some(ConfirmQuitAction::Close) {
        CLOSE_HOVER_FILL_COLOR
    } else {
        CLOSE_FILL_COLOR
    };
    sugarloaf.rounded_rect(
        None,
        layout.close_button.x,
        layout.close_button.y,
        layout.close_button.width,
        layout.close_button.height,
        close_fill,
        DEPTH_BUTTON,
        BUTTON_RADIUS,
        ORDER,
    );

    let title_opts = DrawOpts {
        font_size: TITLE_FONT_SIZE,
        color: TITLE_COLOR,
        ..DrawOpts::default()
    };
    let body_opts = DrawOpts {
        font_size: BODY_FONT_SIZE,
        color: BODY_COLOR,
        ..DrawOpts::default()
    };
    let cancel_opts = DrawOpts {
        font_size: BUTTON_FONT_SIZE,
        color: CANCEL_TEXT_COLOR,
        ..DrawOpts::default()
    };
    let close_opts = DrawOpts {
        font_size: BUTTON_FONT_SIZE,
        color: CLOSE_TEXT_COLOR,
        ..DrawOpts::default()
    };

    let ui = sugarloaf.text_mut();
    draw_centered(
        ui,
        TITLE_TEXT,
        layout.card.x,
        layout.card.width,
        layout.card.y + TITLE_Y_OFFSET,
        &title_opts,
    );
    draw_centered(
        ui,
        BODY_LINE_1,
        layout.card.x,
        layout.card.width,
        layout.card.y + BODY_Y_OFFSET,
        &body_opts,
    );
    draw_centered(
        ui,
        BODY_LINE_2,
        layout.card.x,
        layout.card.width,
        layout.card.y + BODY_Y_OFFSET + BODY_LINE_HEIGHT,
        &body_opts,
    );
    draw_centered_in_rect(ui, CANCEL_TEXT, layout.cancel_button, &cancel_opts);
    draw_centered_in_rect(ui, CLOSE_TEXT, layout.close_button, &close_opts);
}

fn logical_layout(sugarloaf: &Sugarloaf) -> ConfirmQuitLayout {
    let window = sugarloaf.window_size();
    let scale = sugarloaf.scale_factor();
    layout_for_size(window.width / scale, window.height / scale)
}

fn layout_for_size(win_w: f32, win_h: f32) -> ConfirmQuitLayout {
    let max_card_width = (win_w - WINDOW_MARGIN * 2.0).max((win_w - 16.0).max(1.0));
    let card_width = CARD_WIDTH
        .min(max_card_width)
        .max(CARD_MIN_WIDTH.min(max_card_width));
    let max_card_height = (win_h - WINDOW_MARGIN * 2.0).max((win_h - 16.0).max(1.0));
    let card_height = CARD_HEIGHT
        .min(max_card_height)
        .max(156.0_f32.min(max_card_height));
    let card = Rect {
        x: ((win_w - card_width) / 2.0).max(8.0),
        y: ((win_h - card_height) / 2.0).max(8.0),
        width: card_width,
        height: card_height,
    };

    let button_width = ((card.width - CARD_PADDING_X * 2.0 - BUTTON_GAP) / 2.0).max(1.0);
    let button_y = card.y + card.height - BUTTON_BOTTOM_OFFSET - BUTTON_HEIGHT;
    let cancel_button = Rect {
        x: card.x + CARD_PADDING_X,
        y: button_y,
        width: button_width,
        height: BUTTON_HEIGHT,
    };
    let close_button = Rect {
        x: cancel_button.x + button_width + BUTTON_GAP,
        y: button_y,
        width: button_width,
        height: BUTTON_HEIGHT,
    };

    ConfirmQuitLayout {
        card,
        cancel_button,
        close_button,
    }
}

fn stroked_button(
    sugarloaf: &mut Sugarloaf,
    rect: Rect,
    stroke_color: [f32; 4],
    fill_color: [f32; 4],
) {
    sugarloaf.rounded_rect(
        None,
        rect.x,
        rect.y,
        rect.width,
        rect.height,
        stroke_color,
        DEPTH_BUTTON,
        BUTTON_RADIUS,
        ORDER,
    );
    sugarloaf.rounded_rect(
        None,
        rect.x + BUTTON_STROKE,
        rect.y + BUTTON_STROKE,
        rect.width - BUTTON_STROKE * 2.0,
        rect.height - BUTTON_STROKE * 2.0,
        fill_color,
        DEPTH_BUTTON + 0.001,
        (BUTTON_RADIUS - BUTTON_STROKE).max(0.0),
        ORDER,
    );
}

fn draw_centered(
    ui: &mut Text,
    text: &str,
    container_x: f32,
    container_width: f32,
    y: f32,
    opts: &DrawOpts,
) {
    let width = ui.measure(text, opts);
    let x = container_x + ((container_width - width) / 2.0).max(0.0);
    ui.draw(x, y, text, opts);
}

fn draw_centered_in_rect(ui: &mut Text, text: &str, rect: Rect, opts: &DrawOpts) {
    let y = rect.y + ((rect.height - BUTTON_FONT_SIZE) / 2.0) - 1.0;
    draw_centered(ui, text, rect.x, rect.width, y, opts);
}

#[cfg(test)]
// Test lane: default
mod tests {
    use super::*;

    // Defends: the confirm-quit modal remains a real dialog-sized surface, not a tiny tooltip strip.
    #[test]
    fn confirm_quit_dialog_layout_is_centered_and_large() {
        let layout = layout_for_size(900.0, 500.0);

        assert_eq!(layout.card.width, CARD_WIDTH);
        assert_eq!(layout.card.height, CARD_HEIGHT);
        assert_eq!(layout.card.x, 240.0);
        assert_eq!(layout.card.y, 151.0);
    }

    // Defends: drawn button geometry and mouse hit testing share the same layout contract.
    #[test]
    fn confirm_quit_dialog_hit_tests_buttons() {
        let layout = layout_for_size(900.0, 500.0);

        assert!(layout
            .cancel_button
            .contains(layout.cancel_button.x + 1.0, layout.cancel_button.y + 1.0));
        assert!(layout
            .close_button
            .contains(layout.close_button.x + 1.0, layout.close_button.y + 1.0));
        assert!(!layout.cancel_button.contains(0.0, 0.0));
        assert!(!layout.close_button.contains(0.0, 0.0));
    }
}
