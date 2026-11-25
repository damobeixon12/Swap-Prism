use eframe::egui::{self, Color32, RichText, Stroke, Vec2};
pub const BACKGROUND: Color32 = Color32::from_rgb(16, 18, 22);
pub const PANEL: Color32 = Color32::from_rgb(28, 30, 36);
pub const FIELD: Color32 = Color32::from_rgb(38, 41, 48);
pub const TEXT: Color32 = Color32::from_rgb(242, 246, 255);
pub const MUTED: Color32 = Color32::from_rgb(167, 174, 188);
pub const CYAN: Color32 = Color32::from_rgb(208, 211, 217);
pub const PURPLE: Color32 = Color32::from_rgb(187, 191, 199);
pub const GREEN: Color32 = Color32::from_rgb(130, 171, 145);
pub const RED: Color32 = Color32::from_rgb(192, 131, 132);
pub const MAIN_NAV: [(&str, &str); 10] = [
    ("Skins", "SKINS"),
    ("Texturas", "TEXTURAS"),
    ("Fake", "FAKE DISPLAY"),
    ("Overlay", "OVERLAY"),
    ("Tracker", "TRACKER"),
    ("Studio", "PERSONALIZACIÓN"),
    ("Workshop", "WORKSHOP"),
    ("Queue", "CAMBIOS REALIZADOS"),
    ("Community", "DISCORD"),
    ("Settings", "AJUSTES"),
];
pub const FAKE_TABS: [&str; 3] = ["Rank", "Foto de perfil y nombre", "Títulos"];
pub const OVERLAY_TABS: [&str; 3] = ["FPS y ping", "Rank in game", "Soon"];

pub fn setup(ctx: &egui::Context) {
    // set_visuals only edits the active theme. Windows' first light-theme
    // event could otherwise switch to egui's untouched white/black defaults.
    ctx.set_theme(egui::ThemePreference::Dark);
    let mut visual = egui::Visuals::dark();
    visual.override_text_color = Some(TEXT);
    visual.panel_fill = BACKGROUND;
    visual.window_fill = PANEL;
    visual.extreme_bg_color = FIELD;
    visual.faint_bg_color = PANEL;
    visual.selection.bg_fill = Color32::from_rgb(49, 52, 58);
    visual.selection.stroke = Stroke::new(1_f32, CYAN);
    for widget in [
        &mut visual.widgets.inactive,
        &mut visual.widgets.hovered,
        &mut visual.widgets.active,
        &mut visual.widgets.open,
    ] {
        widget.bg_stroke = Stroke::NONE;
        widget.corner_radius = egui::CornerRadius::same(6);
        widget.bg_fill = FIELD;
        widget.weak_bg_fill = FIELD;
        widget.fg_stroke = Stroke::new(1_f32, TEXT);
    }
    visual.widgets.hovered.bg_fill = Color32::from_rgb(49, 53, 62);
    visual.widgets.active.bg_fill = Color32::from_rgb(55, 58, 64);
    visual.widgets.noninteractive.bg_stroke = Stroke::NONE;
    visual.window_stroke = Stroke::NONE;
    visual.window_corner_radius = egui::CornerRadius::same(8);
    ctx.set_visuals(visual);
    let mut style = (*ctx.style()).clone();
    style.spacing.item_spacing = Vec2::new(12., 10.);
    style.spacing.button_padding = Vec2::new(16., 11.);
    style.spacing.interact_size.y = 42.;
    style.animation_time=0.18;
    style
        .text_styles
        .insert(egui::TextStyle::Body, egui::FontId::proportional(16.));
    style
        .text_styles
        .insert(egui::TextStyle::Button, egui::FontId::proportional(15.));
    style
        .text_styles
        .insert(egui::TextStyle::Small, egui::FontId::proportional(13.));
    style
        .text_styles
        .insert(egui::TextStyle::Heading, egui::FontId::proportional(28.));
    ctx.all_styles_mut(|target|*target=style.clone());
    let mut fonts=egui::FontDefinitions::default();
    fonts.font_data.insert("Nunito".into(),egui::FontData::from_static(include_bytes!("../assets/fonts/Nunito-Medium.ttf")).into());
    fonts.families.get_mut(&egui::FontFamily::Proportional).unwrap().insert(0,"Nunito".into());
    if let Ok(bytes)=std::fs::read("C:/Windows/Fonts/segoeui.ttf"){
        fonts.font_data.insert("Symbol fallback".into(),egui::FontData::from_owned(bytes).into());
        fonts.families.get_mut(&egui::FontFamily::Proportional).unwrap().insert(1,"Symbol fallback".into());
    }
    ctx.set_fonts(fonts);
}
pub struct Surface(egui::Frame);
impl Surface {
    pub fn fill(mut self, color: Color32) -> Self {
        self.0 = self.0.fill(color);
        self
    }
    pub fn inner_margin(mut self, margin: impl Into<egui::Margin>) -> Self {
        self.0 = self.0.inner_margin(margin);
        self
    }
    pub fn show<R>(
        self,
        ui: &mut egui::Ui,
        contents: impl FnOnce(&mut egui::Ui) -> R,
    ) -> egui::InnerResponse<R> {
        let response=self.0.show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            contents(ui)
        });
        let hover=ui.rect_contains_pointer(response.response.rect);
        let amount=ui.ctx().animate_bool_with_time(response.response.id.with("card-hover"),hover,0.18);
        if amount>0.01{ui.painter().rect_stroke(response.response.rect,8,Stroke::new(1_f32,MUTED.gamma_multiply(amount*0.3)),egui::StrokeKind::Inside);}
        response
    }
}
pub fn card() -> Surface {
    Surface(
        egui::Frame::new()
            .fill(PANEL)
            .corner_radius(8)
            .inner_margin(20)
            .stroke(Stroke::new(1_f32, Color32::from_rgb(52, 55, 63))),
    )
}
pub fn button(label: &str, tone: Color32) -> egui::Button<'_> {
    egui::Button::new(RichText::new(label).color(tone))
        .fill(FIELD)
        .stroke(Stroke::NONE)
        .corner_radius(6)
        .min_size(Vec2::new(0., 40.))
}
pub fn muted(ui: &mut egui::Ui, text: impl Into<String>) {
    ui.label(RichText::new(text).color(MUTED));
}
pub fn eyebrow(ui: &mut egui::Ui, text: &str, color: Color32) {
    ui.label(RichText::new(text).size(12.).strong().color(color));
}
pub fn search(ui: &mut egui::Ui, value: &mut String, hint: &str) {
    ui.add_sized(
        [ui.available_width(), 38.],
        egui::TextEdit::singleline(value)
            .hint_text(hint)
            .margin(Vec2::new(12., 10.)),
    );
}
pub fn status(entry: &serde_json::Value) -> (&'static str, Color32) {
    match entry["status"].as_str().unwrap_or("") {
        "installed" => ("Instalado", CYAN),
        "pending" => ("Por verificar", PURPLE),
        "restored" | "restored-after-error" => ("Restaurado", MUTED),
        _ => ("Registrado", MUTED),
    }
}
pub fn columns(width: f32) -> usize {
    ((width + 14.) / 224.).floor().max(1.).min(4.) as usize
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn windows_light_theme_cannot_replace_the_dark_design(){
        let ctx=egui::Context::default();
        // Reproduce the former transition: the active dark palette alone
        // leaves light-mode defaults intact when Windows reports Light.
        ctx.set_visuals(egui::Visuals::dark());
        let _=ctx.run(egui::RawInput{system_theme:Some(egui::Theme::Light),..Default::default()},|_|{});
        assert!(!ctx.style().visuals.dark_mode);
        setup(&ctx);
        for theme in [egui::Theme::Light,egui::Theme::Dark,egui::Theme::Light]{
            let _=ctx.run(egui::RawInput{system_theme:Some(theme),..Default::default()},|_|{});
            let style=ctx.style();assert!(style.visuals.dark_mode);
            assert_eq!(style.visuals.widgets.inactive.bg_fill,FIELD);
            assert_eq!(style.visuals.override_text_color,Some(TEXT));
            assert_eq!(style.text_styles[&egui::TextStyle::Body].size,16.);
        }
    }
    #[test]
    fn grid_fits_small_and_large_windows() {
        assert_eq!(columns(180.), 1);
        assert_eq!(columns(680.), 3);
        assert_eq!(columns(1200.), 4);
    }
    #[test]
    fn installed_and_restored_stay_distinct() {
        assert_eq!(
            status(&serde_json::json!({"status":"restored"})).0,
            "Restaurado"
        );
        assert_eq!(
            status(&serde_json::json!({"status":"pending"})).0,
            "Por verificar"
        );
    }
    #[test]
    fn main_navigation_follows_canva() {
        assert_eq!(
            MAIN_NAV.map(|(_, label)| label),
            [
                "SKINS",
                "TEXTURAS",
                "FAKE DISPLAY",
                "OVERLAY",
                "TRACKER",
                "PERSONALIZACIÓN",
                "WORKSHOP",
                "CAMBIOS REALIZADOS",
                  "DISCORD",
                "AJUSTES"
            ]
        );
    }
}
