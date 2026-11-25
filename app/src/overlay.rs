use super::*;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

const TITLE:&str="Swap Prism · Overlay";
const RANK_TITLE:&str="Swap Prism · Rango";

#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Preferences {
    pub fps: bool,
    pub anchor: String,
    pub position: Option<[f32; 2]>,
    pub size: f32,
    pub color: [u8; 3],
    pub rank: bool,
    pub rank_modes: Vec<i64>,
    pub rank_position: Option<[f32; 2]>,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            fps: false,
            anchor: "top-left".into(),
            position: None,
            size: 14.,
            color: [73, 245, 126],
            rank: false,
            rank_modes: vec![10, 11, 13],
            rank_position: None,
        }
    }
}
#[derive(Clone, Default)]
pub struct Placement {
    pub editing: bool,
    pub dragged: Option<[f32; 2]>,
    pub capture: Option<PathBuf>,
    pub captured: bool,
}

/// Own rank per ranked playlist: tier from the save, MMR from Tracker.
#[derive(Default)]
pub struct RankBoard {
    pub tiers: Vec<(i64, usize)>,
    pub mmr: Vec<(i64, String)>,
}
pub const RANK_ROWS: [(i64, &str); 3] = [(10, "1v1"), (11, "2v2"), (13, "3v3")];

/// Shown on its own whenever the game is in front; no background, movable like the FPS counter.
pub fn show_ranks(ctx: &egui::Context, preferences: &Preferences, board: Arc<Mutex<RankBoard>>, placement: Arc<Mutex<Placement>>) {
    let editing = placement.lock().unwrap().editing;
    let rows: Vec<(i64, &str)> = RANK_ROWS.iter().copied().filter(|r| preferences.rank_modes.contains(&r.0)).collect();
    if (!preferences.rank && !editing) || rows.is_empty() {
        platform::overlay_visibility(RANK_TITLE, false);
        return;
    }
    let scale = ctx.pixels_per_point();
    let bounds = platform::game_bounds().unwrap_or([0., 0., 1280. * scale, 860. * scale]);
    let height = rows.len() as f32 * 40. + if editing { 30. } else { 8. };
    let position = preferences.rank_position.map(|p| egui::pos2(p[0], p[1]))
        .unwrap_or(egui::pos2(bounds[0] / scale + 24., (bounds[1] + bounds[3]) / 2. / scale - height / 2.));
    let builder = egui::ViewportBuilder::default()
        .with_title(RANK_TITLE)
        .with_decorations(false)
        .with_transparent(true)
        .with_inner_size([170., height])
        .with_position(position)
        .with_resizable(false)
        .with_taskbar(false)
        .with_active(false)
        .with_mouse_passthrough(!editing)
        .with_window_level(egui::WindowLevel::AlwaysOnTop)
        .with_visible(editing || platform::foreground_game());
    ctx.show_viewport_deferred(egui::ViewportId::from_hash_of("swapprism-rank"), builder, move |ctx, _| {
        ctx.request_repaint_after(Duration::from_millis(400));
        let editing = placement.lock().unwrap().editing;
        platform::transparent_overlay(RANK_TITLE, editing);
        platform::overlay_visibility(RANK_TITLE, editing || platform::foreground_game());
        let board = board.lock().unwrap();
        egui::CentralPanel::default().frame(egui::Frame::new().fill(Color32::TRANSPARENT).inner_margin(4)).show(ctx, |ui| {
            for (id, label) in &rows {
                let tier = board.tiers.iter().find(|t| t.0 == *id).map(|t| t.1).unwrap_or(0);
                let mmr = board.mmr.iter().find(|m| m.0 == *id).map(|m| m.1.as_str()).unwrap_or("—");
                ui.horizontal(|ui| {
                    let (rect, _) = ui.allocate_exact_size(Vec2::splat(32.), egui::Sense::hover());
                    crate::ranks::rank_icon(ui, rect, tier);
                    ui.label(RichText::new(*label).size(14.).strong().color(Color32::from_rgb(225, 230, 238)));
                    ui.label(RichText::new(format!("[{mmr}]")).size(19.).strong().color(design::GREEN()));
                });
            }
            if editing {
                ui.label(RichText::new("Arrastra para mover").size(10.).color(design::MUTED()));
                if ui.interact(ui.max_rect(), ui.id().with("drag"), egui::Sense::drag()).drag_started() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
                }
                if let Some(rect) = ctx.input(|i| i.viewport().outer_rect) {
                    placement.lock().unwrap().dragged = Some([rect.min.x, rect.min.y]);
                }
            }
        });
    });
}

pub fn draw_metrics(ui: &mut egui::Ui, fps: Option<u32>) {
    ui.label(
        RichText::new(format!("{} FPS", fps.map(|v| v.to_string()).unwrap_or("—".into())))
            .font(egui::FontId::proportional(13.))
            .color(design::GREEN()),
    );
}

pub fn show(
    ctx: &egui::Context,
    preferences: &Preferences,
    data: Arc<Mutex<telemetry::Reading>>,
    placement: Arc<Mutex<Placement>>,
) {
    let editing = placement.lock().unwrap().editing;
    let checking = placement.lock().unwrap().capture.is_some();
    if !preferences.fps && !editing && !checking {
        return;
    }
    let scale = ctx.pixels_per_point();
    let bounds = platform::game_bounds().unwrap_or([0., 0., 1280. * scale, 860. * scale]);
    let position = if preferences.anchor == "top-right" {
        egui::pos2(bounds[2] / scale - 184., bounds[1] / scale + 12.)
    } else if preferences.anchor == "bottom-left" {
        egui::pos2(bounds[0] / scale + 14., bounds[3] / scale - 48.)
    } else if preferences.anchor == "bottom-right" {
        egui::pos2(bounds[2] / scale - 184., bounds[3] / scale - 48.)
    } else if preferences.anchor == "custom" {
        preferences
            .position
            .map(|p| egui::pos2(p[0], p[1]))
            .unwrap_or(egui::pos2(bounds[0] / scale + 28., bounds[1] / scale + 28.))
    } else {
        egui::pos2(bounds[0] / scale + 14., bounds[1] / scale + 12.)
    };
    let visible = editing || checking || platform::foreground_game();
    let size = preferences.size.clamp(10., 28.);
    let color = Color32::from_rgb(preferences.color[0], preferences.color[1], preferences.color[2]);
    let builder = egui::ViewportBuilder::default()
        .with_title(TITLE)
        .with_decorations(false)
        .with_transparent(true)
        .with_inner_size([170., if editing { size + 48. } else { size + 12. }])
        .with_position(position)
        .with_resizable(false)
        .with_taskbar(false)
        .with_active(false)
        .with_mouse_passthrough(!editing)
        .with_window_level(egui::WindowLevel::AlwaysOnTop)
        .with_visible(visible);
    ctx.show_viewport_deferred(
        egui::ViewportId::from_hash_of("swapprism-metrics"),
        builder,
        move |ctx, _| {
            ctx.request_repaint_after(Duration::from_millis(150));
            let editing = placement.lock().unwrap().editing;
            let checking = placement.lock().unwrap().capture.is_some();
            platform::transparent_overlay(TITLE,editing);
            platform::overlay_visibility(TITLE,editing || checking || platform::foreground_game());
            let fill = Color32::TRANSPARENT;
            egui::CentralPanel::default()
                .frame(egui::Frame::new().fill(fill).inner_margin(4))
                .show(ctx, |ui| {
                    let fps = data.lock().unwrap().live_fps();
                    ui.label(RichText::new(format!("{} FPS",fps.map(|v|v.to_string()).unwrap_or("—".into()))).font(egui::FontId::proportional(size)).strong().color(color));
                    if editing {
                        ui.label(
                            RichText::new("Arrastra para mover")
                                .size(10.)
                                .color(design::MUTED()),
                        );
                        let response =
                            ui.interact(ui.max_rect(), ui.id().with("drag"), egui::Sense::drag());
                        if response.drag_started() {
                            ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
                        }
                        if let Some(rect) = ctx.input(|i| i.viewport().outer_rect) {
                            placement.lock().unwrap().dragged = Some([rect.min.x, rect.min.y]);
                        }
                    }
                });
            let capture = placement.lock().unwrap().capture.clone();
            if let Some(path) = capture {
                let screenshot = ctx.input(|i| {
                    i.events.iter().find_map(|e| {
                        if let egui::Event::Screenshot { image, .. } = e {
                            Some(image.clone())
                        } else {
                            None
                        }
                    })
                });
                if let Some(image) = screenshot {
                    let bytes: Vec<_> = image.pixels.iter().flat_map(|p| p.to_array()).collect();
                    let _ = image::save_buffer(
                        &path,
                        &bytes,
                        image.size[0] as u32,
                        image.size[1] as u32,
                        image::ColorType::Rgba8,
                    );
                    placement.lock().unwrap().captured = true;
                } else {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(
                        egui::UserData::default(),
                    ));
                }
            }
        },
    );
}

impl App {
    pub fn overlay_controls(&mut self, ui: &mut egui::Ui) {
        if self.overlay_tab == 2 {
            self.pending_feature(ui);
            return;
        }
        if self.overlay_tab == 1 {
            ui.heading("Mi MMR en el juego");
            design::muted(ui,"Tu insignia y tu MMR encima del juego, sin fondo. Solo los tuyos.");
            ui.add_space(12.);
            design::card().show(ui,|ui|{
                ui.horizontal_wrapped(|ui|{
                    let mut changed=ui.checkbox(&mut self.overlay_preferences.rank,"Mostrar mi MMR en el juego").changed();
                    ui.add_space(18.);
                    for(id,label)in RANK_ROWS{
                        let mut on=self.overlay_preferences.rank_modes.contains(&id);
                        if ui.checkbox(&mut on,label).changed(){if on{self.overlay_preferences.rank_modes.push(id);}else{self.overlay_preferences.rank_modes.retain(|m|*m!=id);}changed=true;}
                    }
                    if changed{self.save_overlay();}
                });
                design::muted(ui,"Una vez activado sale solo cada vez que estás en el juego. Marca los modos que quieres ver.");
                let editing=self.rank_placement.lock().unwrap().editing;
                if ui.button(if editing{"Fijar posición"}else{"Mover a cualquier lugar"}).clicked(){
                    let mut placement=self.rank_placement.lock().unwrap();
                    if editing{if let Some(position)=placement.dragged{self.overlay_preferences.rank_position=Some(position);}}
                    placement.editing=!editing;drop(placement);self.save_overlay();
                }
                if editing{design::muted(ui,"Arrastra el panel a donde quieras (por ejemplo junto a tu nombre) y pulsa Fijar posición.");}
                ui.add_space(6.);
                if profile_url_ok(&self.tracker_name){
                    let board=self.rank_board.lock().unwrap();
                    let read=RANK_ROWS.iter().map(|(id,label)|format!("{label} {}",board.mmr.iter().find(|m|m.0==*id).map(|m|m.1.as_str()).unwrap_or("—"))).collect::<Vec<_>>().join("   ");
                    drop(board);
                    design::muted(ui,&format!("Leído de tu Tracker ({}): {read}",self.tracker_name.trim()));
                    design::muted(ui,"Se vuelve a leer cada 2 minutos mientras juegas, así que se actualiza solo al acabar cada partida.");
                }else{
                    ui.label(RichText::new("Primero pon tu nombre de Epic en la sección Tracker > Tracker web.").color(design::RED()));
                }
            });
            return;
        }
        ui.heading("FPS");
        design::muted(
            ui,
            "Un contador pequeño y sin fondo sobre Rocket League.",
        );
        ui.add_space(16.);
        design::card().show(ui,|ui|{
            design::eyebrow(ui,"FPS DEL JUEGO",design::GREEN());
            let before=self.overlay_preferences.fps;ui.checkbox(&mut self.overlay_preferences.fps,"Mostrar FPS");
            if before!=self.overlay_preferences.fps{self.telemetry.enable(self.overlay_preferences.fps);self.save_overlay();}
            let reading=self.telemetry.snapshot();design::muted(ui,if self.overlay_preferences.fps{if reading.live_fps().is_some(){"Midiendo fotogramas del juego".into()}else{reading.status.clone()}}else{"Desactivado".into()});
            ui.add_space(10.);draw_metrics(ui,reading.live_fps());
            design::muted(ui,"Usa ventana sin bordes para ver el overlay externo. Si el juego está minimizado o no hay datos recientes, aparece —.");
        });
        ui.add_space(12.);
        design::card().show(ui, |ui| {
            design::eyebrow(ui, "POSICIÓN", design::CYAN());
            ui.horizontal_wrapped(|ui| {
                if ui.button("Arriba a la izquierda").clicked() {
                    self.overlay_preferences.anchor = "top-left".into();
                    self.save_overlay();
                }
                if ui.button("Arriba a la derecha").clicked() {
                    self.overlay_preferences.anchor = "top-right".into();
                    self.save_overlay();
                }
                for (label, anchor) in [("Abajo a la izquierda", "bottom-left"),("Abajo a la derecha", "bottom-right")] {
                    if ui.button(label).clicked() { self.overlay_preferences.anchor=anchor.into(); self.save_overlay(); }
                }
            });
            let mut changed=false;
            ui.horizontal_wrapped(|ui|{
                ui.label("Tamaño");
                changed |= ui.add(egui::Slider::new(&mut self.overlay_preferences.size,10.0..=28.0).suffix(" px")).changed();
                ui.label("Color"); changed |= ui.color_edit_button_srgb(&mut self.overlay_preferences.color).changed();
            });
            if changed { self.save_overlay(); }
            let editing = self.overlay_placement.lock().unwrap().editing;
            if ui
                .button(if editing {
                    "Fijar posición"
                } else {
                    "Mover a cualquier lugar"
                })
                .clicked()
            {
                let mut placement = self.overlay_placement.lock().unwrap();
                if editing {
                    if let Some(position) = placement.dragged {
                        self.overlay_preferences.position = Some(position);
                        self.overlay_preferences.anchor = "custom".into();
                    }
                }
                placement.editing = !editing;
                drop(placement);
                self.save_overlay();
            }
            if editing {
                design::muted(
                    ui,
                    "Arrastra el indicador flotante y después pulsa Fijar posición.",
                );
            }
        });
    }
    fn save_overlay(&mut self) {
        self.config["overlay"] = serde_json::to_value(&self.overlay_preferences).unwrap();
        if let Err(error) = platform::save_config(&self.root, &self.config) {
            self.status = error;
        }
    }
}
fn profile_url_ok(name: &str) -> bool {
    crate::tracker::profile_url(name).is_ok()
}
