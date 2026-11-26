use super::*;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

const TITLE:&str="Swap Prism · Overlay";
const RANK_TITLE:&str="Swap Prism · Rango";
const BOARD_TITLE:&str="Swap Prism · Marcador";

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
    pub rank_anchor: String,
    pub rank_lobby: bool,
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
            rank_anchor: "left".into(),
            rank_lobby: true,
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
    /// Everyone in a ranked match, drawn next to their row of the in-game scoreboard.
    pub lobby: Vec<LobbyRow>,
}
#[derive(Clone, PartialEq)]
pub struct LobbyRow {
    pub name: String,
    pub team: i64,
    pub score: i64,
    /// Rating, or "…" while it is read.
    pub mmr: String,
    pub tier: usize,
}
pub const RANK_ROWS: [(i64, &str); 3] = [(10, "1v1"), (11, "2v2"), (13, "3v3")];
pub const RANK_ANCHORS: [(&str, &str); 5] = [("left", "Izquierda"), ("top-left", "Arriba a la izquierda"), ("bottom-left", "Abajo a la izquierda"), ("top-right", "Arriba a la derecha"), ("bottom-right", "Abajo a la derecha")];

/// Polls the scoreboard button on its own thread (works while Swap Prism is in the background).
fn watch_scoreboard(ctx: &egui::Context) {
    static STARTED: std::sync::Once = std::sync::Once::new();
    let ctx = ctx.clone();
    STARTED.call_once(move || {
        std::thread::spawn(move || loop {
            let held = platform::scoreboard_held();
            platform::scoreboard_pressed();
            if held != platform::scoreboard_held() { ctx.request_repaint_of(egui::ViewportId::from_hash_of("swapprism-scoreboard")); }
            std::thread::sleep(Duration::from_millis(30));
        });
    });
}

/// Where the rank panel goes; keeps clear of the FPS counter when both share a corner.
fn rank_position(p: &Preferences, bounds: [f32; 4], scale: f32, height: f32) -> egui::Pos2 {
    let (l, t, r, b) = (bounds[0] / scale, bounds[1] / scale, bounds[2] / scale, bounds[3] / scale);
    let fps = if p.fps && p.anchor == p.rank_anchor { p.size + 22. } else { 0. };
    let middle = egui::pos2(l + 24., (t + b) / 2. - height / 2.);
    match p.rank_anchor.as_str() {
        "top-left" => egui::pos2(l + 14., t + 12. + fps),
        "bottom-left" => egui::pos2(l + 14., b - height - 12. - fps),
        "top-right" => egui::pos2(r - 184., t + 12. + fps),
        "bottom-right" => egui::pos2(r - 184., b - height - 12. - fps),
        "custom" => p.rank_position.map(|q| egui::pos2(q[0], q[1])).unwrap_or(middle),
        _ => middle,
    }
}

/// Your own rank and MMR, shown whenever it is enabled and the game is in front; no background.
pub fn show_ranks(ctx: &egui::Context, preferences: &Preferences, board: Arc<Mutex<RankBoard>>, placement: Arc<Mutex<Placement>>) {
    let editing = placement.lock().unwrap().editing;
    let rows: Vec<(i64, &str)> = RANK_ROWS.iter().copied().filter(|r| preferences.rank_modes.contains(&r.0)).collect();
    if (!preferences.rank && !editing) || rows.is_empty() {
        platform::overlay_visibility(RANK_TITLE, false);
        return;
    }
    watch_scoreboard(ctx);
    show_scoreboard(ctx, board.clone());
    let scale = ctx.pixels_per_point();
    let bounds = platform::game_bounds().unwrap_or([0., 0., 1280. * scale, 860. * scale]);
    let height = rows.len() as f32 * 40. + if editing { 30. } else { 8. };
    let position = rank_position(preferences, bounds, scale, height);
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
        let board = board.lock().unwrap();
        platform::overlay_visibility(RANK_TITLE, editing || platform::foreground_game());
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

/// Scoreboard row centres at 1080p, measured on the game's own scoreboard (1v1 and 2v2 captures): the last blue row sits
/// 45 px above the vertical centre, the first orange row 87 px below; rows are 53 px apart and the blue team grows upwards.
const ROW_PITCH: f32 = 53.;
pub fn scoreboard_rows(rows: &[LobbyRow]) -> Vec<(&LobbyRow, f32)> {
    let size = rows.iter().filter(|r| r.team == 0).count().max(rows.iter().filter(|r| r.team == 1).count()).max(1) as f32;
    let mut out = Vec::new();
    for team in [0, 1] {
        let mut members: Vec<&LobbyRow> = rows.iter().filter(|r| r.team == team).collect();
        members.sort_by(|a, b| b.score.cmp(&a.score));
        for (i, row) in members.into_iter().enumerate() {
            let first = if team == 0 { -45. - (size - 1.) * ROW_PITCH } else { 87. };
            out.push((row, first + i as f32 * ROW_PITCH));
        }
    }
    out
}

/// While the scoreboard button is held in a ranked match: rank icon left of each player's photo, MMR after the name.
/// The window lives for the whole match and stays visible (transparent): creating it on each press flashed white, and
/// hiding/showing it behind egui's back stopped its repaints, so a later press showed an unpainted (black) surface.
fn show_scoreboard(ctx: &egui::Context, board: Arc<Mutex<RankBoard>>) {
    let active = !board.lock().unwrap().lobby.is_empty() && platform::foreground_game();
    // Diagnostic (black screen on the scoreboard): window state at every change of what is wanted.
    static LAST: std::sync::Mutex<(bool, bool)> = std::sync::Mutex::new((false, false));
    let note = |wanted: (bool, bool)| {
        let mut last = LAST.lock().unwrap();
        if *last != wanted { *last = wanted; platform::log_overlay(BOARD_TITLE, &format!("activo={} pulsado={}", wanted.0, wanted.1)); }
    };
    let Some(bounds) = platform::game_bounds().filter(|_| active) else {
        note((false, platform::scoreboard_held()));
        platform::overlay_visibility(BOARD_TITLE, false);
        return;
    };
    note((true, platform::scoreboard_held()));
    let scale = ctx.pixels_per_point();
    let builder = egui::ViewportBuilder::default()
        .with_title(BOARD_TITLE)
        .with_decorations(false)
        .with_transparent(true)
        .with_inner_size([(bounds[2] - bounds[0]) / scale, (bounds[3] - bounds[1]) / scale])
        .with_position(egui::pos2(bounds[0] / scale, bounds[1] / scale))
        .with_resizable(false)
        .with_taskbar(false)
        .with_active(false)
        .with_mouse_passthrough(true)
        .with_window_level(egui::WindowLevel::AlwaysOnTop)
        .with_visible(false);
    platform::transparent_overlay(BOARD_TITLE, false);
    ctx.show_viewport_deferred(egui::ViewportId::from_hash_of("swapprism-scoreboard"), builder, move |ctx, _| {
        ctx.request_repaint_after(Duration::from_millis(100));
        platform::transparent_overlay(BOARD_TITLE, false);
        // Shown once the colour key is set; only the rows depend on the button.
        platform::overlay_visibility(BOARD_TITLE, true);
        let show = platform::scoreboard_held();
        static SHOWN: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
        if SHOWN.swap(show, std::sync::atomic::Ordering::Relaxed) != show { platform::log_overlay(BOARD_TITLE, &format!("dibujado mostrar={show}")); }
        egui::CentralPanel::default().frame(egui::Frame::new().fill(Color32::TRANSPARENT)).show(ctx, |ui| {
            let area = ui.max_rect();
            let s = area.height() / 1080.;
            let centre = area.center();
            let board = board.lock().unwrap();
            if !show { return; }
            for (row, y) in scoreboard_rows(&board.lobby) {
                let y = centre.y + y * s;
                // Rank icon just left of the photo; MMR centred in the gap between the name and the PUNTAJE column.
                let icon = egui::Rect::from_center_size(egui::pos2(centre.x - 555. * s, y), Vec2::splat(40. * s));
                crate::ranks::rank_icon(ui, icon, row.tier);
                let at = egui::pos2(centre.x - 149. * s, y);
                // Near-black shadow (pure black is the transparent colour key).
                ui.painter().text(at + Vec2::splat(1.5 * s), egui::Align2::CENTER_CENTER, &row.mmr, egui::FontId::proportional(24. * s), Color32::from_rgb(12, 12, 14));
                ui.painter().text(at, egui::Align2::CENTER_CENTER, &row.mmr, egui::FontId::proportional(24. * s), Color32::WHITE);
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
            design::muted(ui,"Actívalo y listo: detecta tu cuenta y pone tu insignia y tu MMR encima del juego, sin fondo.");
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
                ui.add_space(8.);
                design::muted(ui,"Sale siempre que estás en el juego. El rango de todos aparece junto al marcador mientras mantienes su botón.");
                ui.add_space(8.);
                ui.horizontal_wrapped(|ui|{
                    ui.label("Dónde:");
                    for(anchor,label)in RANK_ANCHORS{if ui.selectable_label(self.overlay_preferences.rank_anchor==anchor,label).clicked(){self.overlay_preferences.rank_anchor=anchor.into();self.save_overlay();}}
                    let editing=self.rank_placement.lock().unwrap().editing;
                    if ui.selectable_label(editing||self.overlay_preferences.rank_anchor=="custom",if editing{"Fijar posición"}else{"Junto a tu nombre (arrastrar)"}).clicked(){
                        let mut placement=self.rank_placement.lock().unwrap();
                        if editing{if let Some(position)=placement.dragged{self.overlay_preferences.rank_position=Some(position);self.overlay_preferences.rank_anchor="custom".into();}}
                        placement.editing=!editing;drop(placement);self.save_overlay();
                    }
                });
                design::muted(ui,"Si coincide con la esquina del contador de FPS, se coloca debajo para no taparlo.");
                ui.add_space(6.);
                if ui.checkbox(&mut self.overlay_preferences.rank_lobby,"MMR del resto de jugadores en partidas igualadas").changed(){self.save_overlay();}
                design::muted(ui,"Se busca su Tracker una vez al empezar y otra al acabar la partida, no durante el juego.");
                let editing=self.rank_placement.lock().unwrap().editing;
                if editing{design::muted(ui,"Arrastra el panel a donde quieras (por ejemplo junto a tu nombre) y pulsa Fijar posición.");}
                ui.add_space(6.);
                if profile_url_ok(&self.tracker_name){
                    design::muted(ui,&format!("Tu cuenta de Epic: {} (detectada automáticamente)",self.tracker_name.trim()));
                    let board=self.rank_board.lock().unwrap();
                    let read=RANK_ROWS.iter().map(|(id,label)|format!("{label} {}",board.mmr.iter().find(|m|m.0==*id).map(|m|m.1.as_str()).unwrap_or("—"))).collect::<Vec<_>>().join("   ");
                    drop(board);
                    design::muted(ui,&format!("Leído de tu Tracker: {read}"));
                    design::muted(ui,"Se actualiza solo al acabar cada partida.");
                }else{
                    ui.label(RichText::new("Abre Rocket League una vez para detectar tu cuenta de Epic (o escríbela en Tracker > Tracker web).").color(design::RED()));
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

#[cfg(test)]
mod tests {
    use super::*;
    fn row(team: i64, score: i64) -> LobbyRow { LobbyRow { name: format!("{team}-{score}"), team, score, mmr: String::new(), tier: 0 } }
    #[test]
    fn one_v_one_matches_measured_rows() {
        let rows = [row(0, 0), row(1, 0)];
        assert_eq!(scoreboard_rows(&rows).iter().map(|r| r.1).collect::<Vec<_>>(), vec![-45., 87.]);
    }
    #[test]
    fn rows_follow_score_order() {
        let rows = [row(0, 10), row(0, 300), row(1, 5)];
        let order: Vec<_> = scoreboard_rows(&rows).iter().map(|r| (r.0.score, r.1)).collect();
        assert_eq!(order, vec![(300, -98.), (10, -45.), (5, 87.)]);
    }
}
