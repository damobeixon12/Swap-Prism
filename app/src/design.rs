use eframe::egui::{self, Color32, RichText, Stroke, Vec2};
use std::sync::atomic::{AtomicUsize,Ordering};
pub struct Theme{pub name:&'static str,bg:[u8;3],side:[u8;3],panel:[u8;3],field:[u8;3],line:[u8;3],text:[u8;3],muted:[u8;3],accent:[u8;3]}
const DARK:([u8;3],[u8;3],[u8;3],[u8;3],[u8;3],[u8;3],[u8;3])=([11,13,16],[14,16,20],[19,22,26],[9,11,14],[34,38,44],[232,234,237],[138,145,156]);
const fn dark(name:&'static str,accent:[u8;3])->Theme{Theme{name,bg:DARK.0,side:DARK.1,panel:DARK.2,field:DARK.3,line:DARK.4,text:DARK.5,muted:DARK.6,accent}}
pub const THEMES:[Theme;6]=[
    dark("Menta",[126,207,160]),
    dark("Océano",[110,172,255]),
    dark("Violeta",[172,142,255]),
    dark("Ámbar",[240,182,92]),
    dark("Coral",[244,128,120]),
    Theme{name:"Medianoche",bg:[8,10,20],side:[11,14,26],panel:[16,20,36],field:[7,9,17],line:[32,38,62],text:[230,234,245],muted:[132,142,170],accent:[120,200,255]},
];
static CURRENT:AtomicUsize=AtomicUsize::new(0);
pub fn set_theme(name:&str)->bool{
    let index=THEMES.iter().position(|t|t.name==name).unwrap_or(0);
    CURRENT.swap(index,Ordering::Relaxed)!=index
}
pub fn theme()->&'static Theme{&THEMES[CURRENT.load(Ordering::Relaxed).min(THEMES.len()-1)]}
fn rgb(c:[u8;3])->Color32{Color32::from_rgb(c[0],c[1],c[2])}
#[allow(non_snake_case)] pub fn BACKGROUND()->Color32{rgb(theme().bg)}
#[allow(non_snake_case)] pub fn SIDEBAR()->Color32{rgb(theme().side)}
#[allow(non_snake_case)] pub fn PANEL()->Color32{rgb(theme().panel)}
#[allow(non_snake_case)] pub fn FIELD()->Color32{rgb(theme().field)}
#[allow(non_snake_case)] pub fn LINE()->Color32{rgb(theme().line)}
#[allow(non_snake_case)] pub fn TEXT()->Color32{rgb(theme().text)}
#[allow(non_snake_case)] pub fn MUTED()->Color32{rgb(theme().muted)}
/// Secondary emphasis (was a light grey "cyan").
#[allow(non_snake_case)] pub fn CYAN()->Color32{TEXT().gamma_multiply(0.86)}
#[allow(non_snake_case)] pub fn PURPLE()->Color32{MUTED()}
/// Accent colour of the active theme.
#[allow(non_snake_case)] pub fn GREEN()->Color32{rgb(theme().accent)}
#[allow(non_snake_case)] pub fn RED()->Color32{Color32::from_rgb(232,128,124)}
/// Dark text drawn on top of the accent colour.
#[allow(non_snake_case)] pub fn ON_ACCENT()->Color32{Color32::from_rgb(10,14,12)}
pub const FAKE_TABS: [&str; 3] = ["Rank", "Foto de perfil y nombre", "Títulos"];
pub const OVERLAY_TABS: [&str; 3] = ["FPS y ping", "Rank in game", "Soon"];

pub fn setup(ctx: &egui::Context) {
    // set_visuals only edits the active theme. Windows' first light-theme
    // event could otherwise switch to egui's untouched white/black defaults.
    ctx.set_theme(egui::ThemePreference::Dark);
    let mut visual = egui::Visuals::dark();
    visual.override_text_color = Some(TEXT());
    visual.panel_fill = BACKGROUND();
    visual.window_fill = PANEL();
    visual.extreme_bg_color = FIELD();
    visual.faint_bg_color = PANEL();
    visual.selection.bg_fill = GREEN().gamma_multiply(0.28);
    visual.selection.stroke = Stroke::new(1_f32, GREEN());
    for widget in [
        &mut visual.widgets.inactive,
        &mut visual.widgets.hovered,
        &mut visual.widgets.active,
        &mut visual.widgets.open,
    ] {
        widget.bg_stroke = Stroke::new(1_f32, LINE());
        widget.corner_radius = egui::CornerRadius::same(8);
        widget.bg_fill = FIELD();
        widget.weak_bg_fill = FIELD();
        widget.fg_stroke = Stroke::new(1_f32, TEXT());
    }
    visual.widgets.hovered.bg_fill = PANEL();
    visual.widgets.hovered.bg_stroke = Stroke::new(1_f32, MUTED().gamma_multiply(0.55));
    visual.widgets.active.bg_fill = LINE();
    visual.widgets.active.bg_stroke = Stroke::new(1_f32, GREEN());
    visual.widgets.noninteractive.bg_stroke = Stroke::NONE;
    visual.window_stroke = Stroke::new(1_f32, LINE());
    visual.window_corner_radius = egui::CornerRadius::same(12);
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
        .insert(egui::TextStyle::Heading, egui::FontId::proportional(20.));
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
        if amount>0.01{ui.painter().rect_stroke(response.response.rect,12,Stroke::new(1_f32,MUTED().gamma_multiply(amount*0.3)),egui::StrokeKind::Inside);}
        response
    }
}
pub fn card() -> Surface {
    Surface(
        egui::Frame::new()
            .fill(PANEL())
            .corner_radius(12)
            .inner_margin(20)
            .stroke(Stroke::new(1_f32, LINE())),
    )
}
pub fn button(label: &str, tone: Color32) -> egui::Button<'_> {
    egui::Button::new(RichText::new(label).color(tone))
        .fill(FIELD())
        .stroke(Stroke::new(1_f32, LINE()))
        .corner_radius(8)
        .min_size(Vec2::new(0., 40.))
}
/// Filled call-to-action in the theme accent («Aplicar»).
pub fn primary(label: &str) -> egui::Button<'_> {
    egui::Button::new(RichText::new(label).color(ON_ACCENT()).strong())
        .fill(GREEN())
        .stroke(Stroke::NONE)
        .corner_radius(8)
        .min_size(Vec2::new(0., 40.))
}
/// Underlined page tabs as in the profile mock-up; returns the clicked index.
pub fn tabs(ui: &mut egui::Ui, labels: &[&str], selected: usize) -> Option<usize> {
    let mut clicked = None;
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = Vec2::new(26., 18.);
        for (i, label) in labels.iter().enumerate() {
            let active = i == selected;
            let text = RichText::new(*label).size(15.).color(if active { TEXT() } else { MUTED() });
            let response = ui.add(egui::Label::new(text).sense(egui::Sense::click())).on_hover_cursor(egui::CursorIcon::PointingHand);
            if active {
                let r = response.rect;
                ui.painter().rect_filled(egui::Rect::from_min_max(egui::pos2(r.left(), r.bottom() + 9.), egui::pos2(r.right(), r.bottom() + 11.)), 1, GREEN());
            }
            if response.clicked() { clicked = Some(i); }
        }
    });
    ui.add_space(10.);
    let y = ui.cursor().min.y;
    ui.painter().hline(ui.max_rect().x_range(), y, Stroke::new(1_f32, LINE()));
    ui.add_space(22.);
    clicked
}
/// Small coloured dot followed by text («● Juego cerrado»).
pub fn dot_label(ui: &mut egui::Ui, text: &str, color: Color32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(10., 18.), egui::Sense::hover());
    ui.painter().circle_filled(rect.center(), 3.5, color);
    ui.label(RichText::new(text).size(14.).color(MUTED()));
}
pub fn muted(ui: &mut egui::Ui, text: impl Into<String>) {
    ui.label(RichText::new(text).color(MUTED()));
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
        "installed" => ("Instalado", CYAN()),
        "pending" => ("Por verificar", PURPLE()),
        "restored" | "restored-after-error" => ("Restaurado", MUTED()),
        _ => ("Registrado", MUTED()),
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
            assert_eq!(style.visuals.widgets.inactive.bg_fill,FIELD());
            assert_eq!(style.visuals.override_text_color,Some(TEXT()));
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
}
