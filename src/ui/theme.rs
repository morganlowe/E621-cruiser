//! Dark theme visuals matching the original E621 Cruiser palette.

use egui::{Color32, CornerRadius, FontId, Stroke, Style, TextStyle, Visuals};

pub const BG_WINDOW: Color32 = Color32::from_rgb(0x16, 0x19, 0x20);
pub const BG_PANEL: Color32 = Color32::from_rgb(0x1a, 0x1d, 0x23);
pub const BG_SURFACE: Color32 = Color32::from_rgb(0x21, 0x25, 0x2b);
pub const BG_HOVER: Color32 = Color32::from_rgb(0x28, 0x2c, 0x34);
pub const BG_IMAGE: Color32 = Color32::from_rgb(0x0b, 0x0d, 0x12);
pub const BORDER_SUBTLE: Color32 = Color32::from_rgb(0x3b, 0x40, 0x48);
pub const ACCENT_BLUE: Color32 = Color32::from_rgb(0x5b, 0x9b, 0xd5);
pub const ACCENT_BLUE_BTN: Color32 = Color32::from_rgb(0x1e, 0x3a, 0x8a);
pub const ACCENT_GREEN_BTN: Color32 = Color32::from_rgb(0x2e, 0x7d, 0x32);
pub const ACCENT_AMBER: Color32 = Color32::from_rgb(0xfb, 0xbf, 0x24);
pub const ACCENT_AMBER_BG: Color32 = Color32::from_rgb(0xd9, 0x77, 0x06);
pub const ACCENT_ARTIST: Color32 = Color32::from_rgb(0xe6, 0xa0, 0x00);
pub const ACCENT_POOL: Color32 = Color32::from_rgb(0xa7, 0x8b, 0xfa);
pub const ACCENT_FAV_BG: Color32 = Color32::from_rgb(0xe1, 0x1d, 0x48);
pub const ACCENT_VOTE_BG: Color32 = Color32::from_rgb(0x16, 0xa3, 0x4a);
pub const ACCENT_RED_BTN: Color32 = Color32::from_rgb(0xb9, 0x1c, 0x1c);
pub const TEXT_PRIMARY: Color32 = Color32::from_rgb(0xd7, 0xda, 0xe0);
pub const TEXT_MUTED: Color32 = Color32::from_rgb(0x9d, 0xa5, 0xb4);
pub const TEXT_ERROR: Color32 = Color32::from_rgb(0xe0, 0x6c, 0x75);
pub const TEXT_SUCCESS: Color32 = Color32::from_rgb(0x48, 0xbb, 0x78);

pub fn configure_style(ctx: &egui::Context) {
    let mut style = Style::default();
    let mut v = Visuals::dark();

    v.window_fill = BG_WINDOW;
    v.panel_fill = BG_PANEL;
    v.faint_bg_color = BG_SURFACE;
    v.extreme_bg_color = BG_IMAGE;
    v.code_bg_color = BG_SURFACE;
    v.override_text_color = Some(TEXT_PRIMARY);
    v.window_stroke = Stroke::new(1.0_f32, BORDER_SUBTLE);
    v.window_corner_radius = CornerRadius::same(6);

    v.widgets.noninteractive.bg_fill = BG_PANEL;
    v.widgets.noninteractive.weak_bg_fill = BG_SURFACE;
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, BORDER_SUBTLE);
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, TEXT_PRIMARY);

    v.widgets.inactive.bg_fill = BG_SURFACE;
    v.widgets.inactive.weak_bg_fill = BG_SURFACE;
    v.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, BORDER_SUBTLE);
    v.widgets.inactive.fg_stroke = Stroke::new(1.0_f32, TEXT_PRIMARY);
    v.widgets.inactive.corner_radius = CornerRadius::same(4);

    v.widgets.hovered.bg_fill = BG_HOVER;
    v.widgets.hovered.weak_bg_fill = BG_HOVER;
    v.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, ACCENT_BLUE);
    v.widgets.hovered.fg_stroke = Stroke::new(1.0_f32, Color32::WHITE);
    v.widgets.hovered.corner_radius = CornerRadius::same(4);

    v.widgets.active.bg_fill = Color32::from_rgb(0x2b, 0x33, 0x42);
    v.widgets.active.weak_bg_fill = Color32::from_rgb(0x2b, 0x33, 0x42);
    v.widgets.active.bg_stroke = Stroke::new(1.0_f32, ACCENT_BLUE);
    v.widgets.active.fg_stroke = Stroke::new(1.0_f32, Color32::WHITE);

    v.selection.bg_fill = Color32::from_rgb(0x26, 0x4f, 0x78);
    v.selection.stroke = Stroke::new(1.5_f32, ACCENT_BLUE);

    style.visuals = v;
    style.text_styles.insert(TextStyle::Small, FontId::proportional(11.5));
    style.text_styles.insert(TextStyle::Body, FontId::proportional(13.5));
    style.text_styles.insert(TextStyle::Button, FontId::proportional(13.0));
    style.text_styles.insert(TextStyle::Monospace, FontId::monospace(12.0));
    style.text_styles.insert(TextStyle::Heading, FontId::proportional(16.0));
    style.spacing.item_spacing = egui::vec2(6.0, 6.0);
    style.spacing.button_padding = egui::vec2(8.0, 4.0);
    ctx.set_style(style);
}
