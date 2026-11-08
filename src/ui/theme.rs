//! Shared dark palette for controls, presentation, and local map canvases.

use eframe::egui;
use egui::Color32;

const BG: Color32 = Color32::from_rgb(11, 13, 18);
const WIDGET: Color32 = Color32::from_rgb(22, 26, 35);
const WIDGET_HOVER: Color32 = Color32::from_rgb(35, 41, 55);
const BORDER: Color32 = Color32::from_rgb(38, 42, 53);
const ACCENT: Color32 = Color32::from_rgb(139, 158, 255);
const ACCENT_DIM: Color32 = Color32::from_rgb(106, 126, 224);
const TEXT: Color32 = Color32::from_rgb(245, 245, 247);
const TEXT_DIM: Color32 = Color32::from_rgb(156, 163, 175);

pub fn background() -> Color32 {
    BG
}
pub fn border() -> Color32 {
    BORDER
}
pub fn map_background() -> Color32 {
    Color32::from_rgb(15, 19, 27)
}
pub fn road() -> Color32 {
    Color32::from_rgb(100, 113, 131)
}
pub fn building() -> Color32 {
    Color32::from_rgb(98, 82, 74)
}
pub fn route() -> Color32 {
    ACCENT
}

pub fn ok() -> Color32 {
    Color32::from_rgb(77, 193, 143)
}
pub fn warn() -> Color32 {
    Color32::from_rgb(232, 185, 88)
}
pub fn err() -> Color32 {
    Color32::from_rgb(245, 116, 126)
}
#[allow(dead_code)]
pub fn accent() -> Color32 {
    ACCENT
}
pub fn text_dim() -> Color32 {
    TEXT_DIM
}

pub fn plain() -> Color32 {
    TEXT_DIM
}
pub fn text() -> Color32 {
    TEXT
}

/// Accent buttons use dark text to retain contrast on the light accent.
pub fn primary_btn(text: &str) -> egui::Button<'static> {
    egui::Button::new(egui::RichText::new(text.to_owned()).color(BG).strong())
        .fill(ACCENT)
        .stroke(egui::Stroke::NONE)
        .rounding(egui::Rounding::same(6.0))
}

pub fn external_link(
    ui: &mut egui::Ui,
    label: impl Into<egui::WidgetText>,
    url: impl Into<String>,
) -> egui::Response {
    ui.add_enabled(
        !crate::runtime::offline(),
        egui::Hyperlink::from_label_and_url(label, url.into()),
    )
}

pub fn apply(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();
    let v = &mut style.visuals;
    *v = egui::Visuals::dark();

    v.panel_fill = BG;
    v.window_fill = BG;
    v.extreme_bg_color = Color32::from_rgb(7, 9, 13);
    v.faint_bg_color = Color32::from_rgb(18, 22, 30);
    v.override_text_color = Some(TEXT);
    v.hyperlink_color = ACCENT;
    v.selection.bg_fill = ACCENT;
    v.selection.stroke = egui::Stroke::new(1.0_f32, BG);
    v.window_stroke = egui::Stroke::new(1.0_f32, BORDER);

    v.widgets.noninteractive.bg_fill = WIDGET;
    v.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.1_f32, TEXT);
    v.widgets.inactive.bg_fill = WIDGET;
    v.widgets.inactive.weak_bg_fill = WIDGET;
    v.widgets.inactive.fg_stroke = egui::Stroke::new(1.0_f32, TEXT_DIM);
    v.widgets.inactive.bg_stroke = egui::Stroke::new(1.0_f32, BORDER);
    v.widgets.hovered.bg_fill = WIDGET_HOVER;
    v.widgets.hovered.weak_bg_fill = WIDGET_HOVER;
    v.widgets.hovered.fg_stroke = egui::Stroke::new(1.2_f32, TEXT);
    v.widgets.hovered.bg_stroke = egui::Stroke::new(1.0_f32, ACCENT);
    v.widgets.active.bg_fill = ACCENT_DIM;
    v.widgets.active.weak_bg_fill = ACCENT_DIM;
    v.widgets.active.fg_stroke = egui::Stroke::new(1.3_f32, Color32::WHITE);
    v.widgets.open.bg_fill = WIDGET;
    v.widgets.open.weak_bg_fill = WIDGET;
    v.widgets.open.fg_stroke = egui::Stroke::new(1.2_f32, TEXT);
    for w in [
        &mut v.widgets.noninteractive,
        &mut v.widgets.inactive,
        &mut v.widgets.hovered,
        &mut v.widgets.active,
        &mut v.widgets.open,
    ] {
        w.rounding = egui::Rounding::same(6.0);
    }

    for style_key in [
        egui::TextStyle::Heading,
        egui::TextStyle::Body,
        egui::TextStyle::Button,
        egui::TextStyle::Small,
    ] {
        let font_id = style.text_styles.get_mut(&style_key).unwrap();
        // 保持字号不变，只确保颜色由 override_text_color 控制
        let _ = font_id;
    }

    style.spacing.item_spacing = egui::vec2(10.0, 7.0);
    style.spacing.button_padding = egui::vec2(12.0, 5.0);
    style.spacing.menu_margin = egui::Margin::same(8.0);
    #[cfg(target_os = "android")]
    {
        style.spacing.interact_size = egui::vec2(44.0, 44.0);
        style.spacing.button_padding = egui::vec2(14.0, 10.0);
    }

    // 页面文字仅作展示：不可选中、不可复制，输入框不受影响
    style.interaction.selectable_labels = false;
    style.interaction.multi_widget_text_select = false;

    ctx.set_style(style);
}
