//! Comfort: first-run guide, «what's new», drag and drop, big skin preview, interface size, start with Windows
//! and freeing disk space.
use super::*;
use crate::design as d;

/// Shown once to everyone who has not seen this set of news yet.
pub const NEWS_VERSION: &str = "0.33.0";
const NEWS: [(&str, &str); 4] = [
    ("Ctrl + K", "Busca cualquier skin, textura, sonido o mapa y ve directo."),
    ("Favoritos", "Toca la estrella de lo que más usas y fíltralo."),
    ("Conjuntos", "Guarda lo que tienes puesto, cámbialo de un clic y compártelo. En Cambios."),
    ("Aplicar al cerrar el juego", "Pulsa Aplicar con el juego abierto y se hace solo al cerrarlo."),
];
pub const SIZES: [(&str, f32); 3] = [("Pequeño", 0.9), ("Normal", 1.0), ("Grande", 1.15)];

const GUIDE: [(&str, &str); 3] = [
    ("Bienvenido a Swap Prism", "Cambia skins, sonidos y tu perfil de Rocket League. Los cambios solo los ves tú, en tu PC."),
    ("Elige tu primer cambio", "En Skins eliges un objeto tuyo y cómo quieres que se vea. En Texturas y en Sonidos y colores hay más."),
    ("Aplica con el juego cerrado", "Abajo aparece la barra Aplicar. Si el juego está abierto, se aplica solo al cerrarlo. En Cambios vuelves al original cuando quieras."),
];

fn human(bytes: u64) -> String {
    let mb = bytes as f64 / 1_048_576.;
    if mb >= 1024. { format!("{:.1} GB", mb / 1024.) } else if mb >= 10. { format!("{mb:.0} MB") } else { format!("{mb:.1} MB") }
}

impl App {
    pub(super) fn pick_game_folder(&mut self) {
        if let Some(path) = rfd::FileDialog::new().set_title("Elige rocketleague (contiene Binaries y TAGame)").pick_folder() {
            if platform::valid_game(&path) {
                self.game = Some(path.clone());
                self.config["game_path"] = json!(path);
                self.status = platform::save_config(&self.root, &self.config).err().unwrap_or("Ruta guardada.".into());
            } else {
                self.status = "Elige la carpeta rocketleague que contiene Binaries y TAGame.".into();
            }
        }
    }

    fn remember(&mut self, key: &str, value: Value) {
        self.config[key] = value;
        if let Err(e) = platform::save_config(&self.root, &self.config) { self.status = e; }
    }

    // ---------- first run and news ----------
    pub(super) fn welcome(&mut self, ctx: &egui::Context) {
        if (self.shot.is_some() && std::env::var_os("SP_SHOT_WELCOME").is_none()) || self.design_dir.is_some() { return; }
        if let Some(step) = self.guide_step { self.guide(ctx, step); } else if self.news_open { self.news(ctx); }
    }

    fn modal_frame() -> egui::Frame {
        egui::Frame::new().fill(d::PANEL()).corner_radius(16).inner_margin(28).stroke(egui::Stroke::new(1_f32, d::LINE()))
    }

    fn guide(&mut self, ctx: &egui::Context, step: usize) {
        let (mut next, mut skip) = (false, false);
        egui::Modal::new(egui::Id::new("first-run")).frame(Self::modal_frame()).show(ctx, |ui| {
            ui.set_width(460.);
            ui.horizontal(|ui| {
                for i in 0..GUIDE.len() {
                    let (rect, _) = ui.allocate_exact_size(Vec2::new(if i == step { 22. } else { 8. }, 8.), egui::Sense::hover());
                    ui.painter().rect_filled(rect, 4, if i == step { d::GREEN() } else { d::LINE() });
                }
            });
            ui.add_space(18.);
            ui.label(RichText::new(GUIDE[step].0).size(24.).strong());
            ui.add_space(8.);
            ui.label(RichText::new(GUIDE[step].1).size(15.).color(d::MUTED()));
            ui.add_space(18.);
            if step == 0 {
                match self.game.clone() {
                    Some(path) => { ui.horizontal(|ui| { d::dot_label(ui, "Juego encontrado", d::GREEN()); }); d::muted(ui, path.display().to_string()); }
                    None => if ui.add(d::button("Elegir carpeta del juego", d::TEXT())).clicked() { self.pick_game_folder(); },
                }
                ui.add_space(18.);
            }
            if step == GUIDE.len() - 1 { d::muted(ui, "Consejo: Ctrl + K busca cualquier cosa."); ui.add_space(18.); }
            ui.horizontal(|ui| {
                if ui.add(egui::Button::new(RichText::new("Saltar").color(d::MUTED())).frame(false)).clicked() { skip = true; }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.add(d::primary(if step + 1 == GUIDE.len() { "Ir a Skins" } else { "Siguiente" })).clicked() { next = true; }
                });
            });
        });
        if next && step + 1 < GUIDE.len() { self.guide_step = Some(step + 1); return; }
        if next || skip {
            self.guide_step = None;
            self.remember("onboarded", json!(true));
            self.remember("seen_news", json!(NEWS_VERSION));
            if next { self.go_to("Skins"); }
        }
    }

    fn news(&mut self, ctx: &egui::Context) {
        let mut close = false;
        let response = egui::Modal::new(egui::Id::new("news")).frame(Self::modal_frame()).show(ctx, |ui| {
            ui.set_width(440.);
            ui.label(RichText::new("Novedades").size(24.).strong());
            d::muted(ui, format!("Swap Prism {VERSION}"));
            ui.add_space(16.);
            for (title, text) in NEWS {
                ui.horizontal(|ui| {
                    let (rect, _) = ui.allocate_exact_size(Vec2::new(10., 20.), egui::Sense::hover());
                    ui.painter().circle_filled(rect.center(), 4., d::GREEN());
                    ui.vertical(|ui| { ui.label(RichText::new(title).size(15.).strong()); ui.label(RichText::new(text).size(14.).color(d::MUTED())); });
                });
                ui.add_space(10.);
            }
            ui.add_space(6.);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| { if ui.add(d::primary("Entendido")).clicked() { close = true; } });
        });
        if close || response.should_close() { self.news_open = false; self.remember("seen_news", json!(NEWS_VERSION)); }
    }

    // ---------- drag and drop ----------
    /// A dropped file goes where it belongs: MP3 to sounds, pictures to the profile or textures, ZIP/DDS to textures, maps to Mapas.
    pub(super) fn dropped_files(&mut self, ctx: &egui::Context) {
        let (hovering, dropped) = ctx.input(|i| (!i.raw.hovered_files.is_empty(), i.raw.dropped_files.iter().filter_map(|f| f.path.clone()).collect::<Vec<_>>()));
        if hovering {
            let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::new("drop")));
            let screen = ctx.screen_rect();
            painter.rect_filled(screen, 0, Color32::from_black_alpha(170));
            painter.rect_stroke(screen.shrink(24.), 18, egui::Stroke::new(2_f32, d::GREEN().gamma_multiply(0.7)), egui::StrokeKind::Inside);
            painter.text(screen.center() - Vec2::new(0., 14.), egui::Align2::CENTER_CENTER, "Suelta para añadir", d::bold(28.), d::TEXT());
            painter.text(screen.center() + Vec2::new(0., 22.), egui::Align2::CENTER_CENTER, "MP3, imágenes, texturas (ZIP, DDS) o mapas (UDK, UPK)", egui::FontId::proportional(15.), d::MUTED());
        }
        if let Some(path) = dropped.into_iter().next() { self.open_dropped(path); }
    }

    fn open_dropped(&mut self, path: PathBuf) {
        if self.busy { self.status = "Espera a que termine lo que está haciendo la app y vuelve a soltarlo.".into(); return; }
        let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
        let picture = ["png", "jpg", "jpeg", "webp"].contains(&ext.as_str());
        match ext.as_str() {
            "mp3" => {
                let kind = if self.section == "Studio" && self.studio_tab == 1 { self.studio_sound_kind.clone() } else { "reset".into() };
                self.go_to("Studio");
                self.studio_tab = 1;
                self.studio_sound_kind = kind.clone();
                self.call("sound_import", json!({"action": "sound_import", "path": path, "kind": kind}));
            }
            _ if picture && self.section == "Fake" => { self.fake_tab = 1; self.open_crop("avatar", &path.display().to_string(), None); }
            "udk" | "upk" => { self.go_to("Workshop"); self.call("workshop_import", json!({"action": "workshop_import", "path": path})); }
            "zip" if self.section == "Workshop" => self.call("workshop_import", json!({"action": "workshop_import", "path": path})),
            "zip" | "dds" | "json" => { self.go_to("Texturas"); self.texture_shop = false; self.call("import", json!({"action": "import", "path": path})); }
            _ if picture && self.section == "Texturas" => self.call("import", json!({"action": "import", "path": path})),
            _ if picture => self.status = "Suelta la imagen en Perfil (foto) o en Texturas.".into(),
            _ if path.is_dir() => { self.go_to("Texturas"); self.texture_shop = false; self.call("import", json!({"action": "import", "path": path})); }
            _ => self.status = "Ese tipo de archivo no se puede usar. Prueba con un MP3, una imagen, un ZIP de texturas o un mapa.".into(),
        }
    }

    // ---------- big preview ----------
    /// The 256 px picture of a catalog item, downloaded once into data/thumbnails-large (public images, no user data).
    pub(super) fn large_image(&mut self, ctx: &egui::Context, url: &str) -> Option<PathBuf> {
        use sha2::{Digest, Sha256};
        if !url.starts_with("https://cdn.source.gs/") { return None; }
        let path = self.root.join(format!("data/thumbnails-large/{:x}.webp", Sha256::digest(url.as_bytes())));
        if path.is_file() { return Some(path); }
        if self.large_requested.insert(url.to_owned()) {
            let (url, ctx) = (url.to_owned(), ctx.clone());
            thread::spawn(move || {
                let fetch = || -> Result<(), String> {
                    let tls = native_tls::TlsConnector::new().map_err(|e| e.to_string())?;
                    let agent = ureq::AgentBuilder::new().redirects(0).timeout(Duration::from_secs(15)).user_agent("SwapPrism (item preview)").tls_connector(std::sync::Arc::new(tls)).build();
                    let mut bytes = Vec::new();
                    std::io::Read::read_to_end(&mut std::io::Read::take(agent.get(&url).call().map_err(|e| e.to_string())?.into_reader(), 2_000_001), &mut bytes).map_err(|e| e.to_string())?;
                    if bytes.len() > 2_000_000 || bytes.get(8..12) != Some(b"WEBP") { return Err("not a webp".into()); }
                    fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
                    let tmp = path.with_extension("tmp");
                    fs::write(&tmp, &bytes).map_err(|e| e.to_string())?;
                    fs::rename(&tmp, &path).map_err(|e| e.to_string())
                };
                if fetch().is_ok() { ctx.request_repaint(); }
            });
        }
        None
    }

    /// Hover card of a skin: big picture and its details.
    pub(super) fn item_hover(&mut self, ui: &mut egui::Ui, row: &Value, note: &str) {
        let large = self.large_image(ui.ctx(), s(row, "Image"));
        let small = platform::thumbnail_path(&self.root, s(row, "Image"));
        let picture = large.or(Some(small).filter(|p| p.is_file()));
        if let Some(path) = picture {
            ui.add(egui::Image::from_uri(format!("file://{}", path.display())).fit_to_exact_size(Vec2::splat(240.)).corner_radius(10));
        }
        ui.label(RichText::new(s(row, "Product")).strong().size(15.));
        d::muted(ui, note);
    }

    // ---------- Ajustes ----------
    pub(super) fn comfort_settings(&mut self, ui: &mut egui::Ui) {
        d::card().show(ui, |ui| {
            d::eyebrow(ui, "INTERFAZ", d::GREEN());
            ui.heading("Tamaño");
            ui.add_space(10.);
            let current = self.config["ui_scale"].as_f64().unwrap_or(1.) as f32;
            let selected = SIZES.iter().position(|(_, z)| (*z - current).abs() < 0.01).unwrap_or(1);
            if let Some(i) = d::tabs(ui, &SIZES.map(|(n, _)| n), selected) {
                ui.ctx().set_zoom_factor(SIZES[i].1);
                self.remember("ui_scale", json!(SIZES[i].1));
            }
        });
        ui.add_space(12.);
        d::card().show(ui, |ui| {
            d::eyebrow(ui, "INICIO", d::GREEN());
            ui.heading("Segundo plano");
            ui.add_space(10.);
            let mut on = self.autostart;
            if d::toggle(ui, &mut on, "Abrir con Windows, oculta en la bandeja").changed() {
                match platform::set_autostart(on) { Ok(()) => self.autostart = on, Err(e) => self.status = e }
            }
            ui.add_space(4.);
            d::muted(ui, "Así el overlay y «Aplicar al cerrar el juego» funcionan sin abrirla. Muéstrala con F8 o con su icono junto al reloj.");
        });
        ui.add_space(12.);
        if self.storage.is_null() && !self.busy { self.storage = json!({"loading": true}); self.call("storage", json!({"action": "storage"})); }
        d::card().show(ui, |ui| {
            d::eyebrow(ui, "DISCO", d::GREEN());
            ui.heading("Espacio");
            ui.add_space(10.);
            if self.storage["loading"] == true { ui.spinner(); return; }
            let row = |ui: &mut egui::Ui, label: &str, bytes: &Value, note: &str| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(label).size(15.));
                    d::muted(ui, note);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| { ui.label(RichText::new(human(bytes.as_u64().unwrap_or(0))).strong()); });
                });
                ui.add_space(6.);
            };
            row(ui, "Copias de los originales", &self.storage["backups"], "se conservan");
            row(ui, "Tu biblioteca", &self.storage["library"], "texturas, sonidos, mapas y fotos");
            row(ui, "Temporales", &self.storage["temporary"], "se pueden borrar");
            ui.add_space(4.);
            let can = !self.busy && !self.update_downloading && self.update_file.is_none() && self.storage["temporary"].as_u64().unwrap_or(0) > 0;
            if ui.add_enabled(can, d::button("Liberar espacio", d::TEXT())).clicked() { self.call("storage_clean", json!({"action": "storage_clean"})); }
        });
    }

    /// Engine results of «storage» and «storage_clean».
    pub(super) fn storage_result(&mut self, action: &str, value: Value) {
        if action == "storage_clean" { self.status = format!("Listo: {} liberados.", human(value["freed"].as_u64().unwrap_or(0))); }
        self.storage = value;
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn sizes_read_well() {
        assert_eq!(super::human(5 * 1_048_576), "5.0 MB");
        assert_eq!(super::human(3 * 1_073_741_824), "3.0 GB");
    }
}
