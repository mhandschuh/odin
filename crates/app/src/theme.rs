use eframe::egui::{
    self, Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Margin, Stroke, Theme, Visuals,
};
use std::sync::Arc;

pub const FILL: &str = "phosphor-fill";

// Pantheon's Mac palette, shared with Heimdall; not every color is used here.
#[allow(dead_code)]
#[derive(Clone, Copy)]
pub struct Palette {
    pub dark: bool,
    pub window: Color32,
    pub sidebar: Color32,
    pub content: Color32,
    pub stripe: Color32,
    pub toolbar: Color32,
    pub separator: Color32,
    pub text: Color32,
    pub secondary: Color32,
    pub tertiary: Color32,
    pub accent: Color32,
    pub selection: Color32,
    pub selection_inactive: Color32,
    pub selection_text: Color32,
    pub hover: Color32,
    pub control: Color32,
    pub folder: Color32,
    pub danger: Color32,
}

pub fn palette(dark: bool) -> Palette {
    if dark {
        Palette {
            dark,
            window: Color32::from_rgb(38, 38, 40),
            sidebar: Color32::from_rgb(41, 41, 43),
            content: Color32::from_rgb(30, 30, 31),
            stripe: Color32::from_rgb(36, 36, 38),
            toolbar: Color32::from_rgb(44, 44, 46),
            separator: Color32::from_rgb(58, 58, 60),
            text: Color32::from_rgb(229, 229, 231),
            secondary: Color32::from_rgb(152, 152, 157),
            tertiary: Color32::from_rgb(99, 99, 102),
            // Ember, the Pantheon apps' accent (Vesta's on Android), in place of macOS blue.
            accent: Color32::from_rgb(240, 144, 74),
            selection: Color32::from_rgb(178, 92, 36),
            selection_inactive: Color32::from_rgb(70, 70, 73),
            selection_text: Color32::WHITE,
            hover: Color32::from_white_alpha(14),
            control: Color32::from_rgb(58, 58, 61),
            folder: Color32::from_rgb(88, 166, 236),
            danger: Color32::from_rgb(255, 105, 97),
        }
    } else {
        Palette {
            dark,
            window: Color32::from_rgb(246, 246, 246),
            sidebar: Color32::from_rgb(234, 234, 236),
            content: Color32::WHITE,
            stripe: Color32::from_rgb(245, 245, 247),
            toolbar: Color32::from_rgb(250, 250, 250),
            separator: Color32::from_rgb(222, 222, 224),
            text: Color32::from_rgb(29, 29, 31),
            secondary: Color32::from_rgb(110, 110, 115),
            tertiary: Color32::from_rgb(174, 174, 178),
            accent: Color32::from_rgb(214, 108, 36),
            selection: Color32::from_rgb(196, 98, 30),
            selection_inactive: Color32::from_rgb(220, 220, 224),
            selection_text: Color32::WHITE,
            hover: Color32::from_black_alpha(12),
            control: Color32::from_rgb(227, 227, 229),
            folder: Color32::from_rgb(78, 160, 236),
            danger: Color32::from_rgb(215, 50, 40),
        }
    }
}

pub fn current(ctx: &egui::Context) -> Palette {
    palette(ctx.global_style().visuals.dark_mode)
}

pub fn font(size: f32) -> FontId {
    FontId::proportional(size)
}

pub fn fill_font(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(FILL.into()))
}

pub fn install(ctx: &egui::Context) {
    install_fonts(ctx);
    match std::env::var("YGGDRASIL_THEME").as_deref() {
        Ok("light") => ctx.set_theme(Theme::Light),
        Ok("dark") => ctx.set_theme(Theme::Dark),
        _ => {}
    }
    for theme in [Theme::Light, Theme::Dark] {
        ctx.style_mut_of(theme, |style| {
            let p = palette(theme == Theme::Dark);
            let mut v = if p.dark { Visuals::dark() } else { Visuals::light() };
            v.panel_fill = p.content;
            v.window_fill = if p.dark { Color32::from_rgb(44, 44, 46) } else { Color32::from_rgb(252, 252, 252) };
            v.window_stroke = Stroke::new(1.0, p.separator);
            v.window_corner_radius = CornerRadius::same(12);
            v.window_shadow.blur = 30;
            v.window_shadow.color = Color32::from_black_alpha(if p.dark { 120 } else { 45 });
            v.popup_shadow.blur = 16;
            v.popup_shadow.color = Color32::from_black_alpha(if p.dark { 110 } else { 35 });
            v.menu_corner_radius = CornerRadius::same(8);
            v.extreme_bg_color = p.control;
            v.faint_bg_color = p.stripe;
            v.override_text_color = Some(p.text);
            v.hyperlink_color = p.accent;
            v.selection.bg_fill = p.accent.gamma_multiply(0.35);
            v.selection.stroke = Stroke::new(1.0, p.accent);
            v.text_cursor.stroke = Stroke::new(1.5, p.accent);
            v.striped = false;
            v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, p.separator);
            v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, p.text);
            v.widgets.inactive.weak_bg_fill = p.control;
            v.widgets.inactive.bg_fill = p.control;
            v.widgets.inactive.bg_stroke = Stroke::NONE;
            v.widgets.inactive.fg_stroke = Stroke::new(1.0, p.text);
            v.widgets.hovered.weak_bg_fill = p.control.gamma_multiply(if p.dark { 1.25 } else { 0.94 });
            v.widgets.hovered.bg_fill = v.widgets.hovered.weak_bg_fill;
            v.widgets.hovered.bg_stroke = Stroke::NONE;
            v.widgets.hovered.fg_stroke = Stroke::new(1.0, p.text);
            v.widgets.active.weak_bg_fill = p.control.gamma_multiply(if p.dark { 1.5 } else { 0.88 });
            v.widgets.active.bg_fill = v.widgets.active.weak_bg_fill;
            v.widgets.active.bg_stroke = Stroke::NONE;
            v.widgets.active.fg_stroke = Stroke::new(1.0, p.text);
            v.widgets.open = v.widgets.hovered;
            for w in [
                &mut v.widgets.noninteractive,
                &mut v.widgets.inactive,
                &mut v.widgets.hovered,
                &mut v.widgets.active,
                &mut v.widgets.open,
            ] {
                w.corner_radius = CornerRadius::same(6);
                w.expansion = 0.0;
            }
            style.visuals = v;
            style.spacing.item_spacing = egui::vec2(8.0, 6.0);
            style.spacing.button_padding = egui::vec2(10.0, 4.0);
            style.spacing.interact_size.y = 24.0;
            style.spacing.menu_margin = Margin::same(5);
            style.spacing.window_margin = Margin::same(18);
            style.interaction.selectable_labels = false;
            style.interaction.tooltip_delay = 0.6;
            for (ts, font) in style.text_styles.iter_mut() {
                font.size = match ts {
                    egui::TextStyle::Heading => 16.0,
                    egui::TextStyle::Small => 11.0,
                    _ => 13.0,
                };
            }
        });
    }
}

fn install_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    let candidates = ["/System/Library/Fonts/SFNS.ttf", "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf"];
    if let Some(bytes) = candidates.iter().find_map(|p| std::fs::read(p).ok()) {
        fonts.font_data.insert("system".into(), Arc::new(FontData::from_owned(bytes)));
        fonts.families.entry(FontFamily::Proportional).or_default().insert(0, "system".into());
    }
    egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);
    egui_phosphor::add_font_bytes_as_family(&mut fonts, FILL, egui_phosphor::Variant::Fill.font_bytes());
    ctx.set_fonts(fonts);
}
