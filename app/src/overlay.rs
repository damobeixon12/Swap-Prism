use super::*;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Preferences {
    pub fps: bool,
    pub anchor: String,
    pub position: Option<[f32; 2]>,
    pub size: f32,
    pub color: [u8; 3],
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            fps: false,
            anchor: "top-left".into(),
            position: None,
            size: 14.,
            color: [73, 245, 126],
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

pub fn draw_metrics(ui: &mut egui::Ui, fps: Option<u32>, ping: Option<u32>, show_ping: bool) {
    let green = design::GREEN();
    ui.label(
        RichText::new(format!(
            "{} FPS",
            fps.map(|v| v.to_string()).unwrap_or("—".into())
        ))
        .font(egui::FontId::proportional(13.))
        .color(green),
    );
    if show_ping {
        ui.horizontal(|ui| {
            let (rect, _) = ui.allocate_exact_size(Vec2::new(22., 18.), egui::Sense::hover());
            for i in 0..4 {
                let h = 4. + i as f32 * 4.;
                let r = egui::Rect::from_min_size(
                    egui::pos2(rect.left() + i as f32 * 5., rect.bottom() - h),
                    Vec2::new(3., h),
                );
                ui.painter().rect_filled(r, 1, green);
            }
            ui.label(
                RichText::new(ping.map(|v| v.to_string()).unwrap_or("—".into()))
                    .font(egui::FontId::proportional(13.))
                    .color(green),
            );
        });
    }
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
        .with_title("Swap Prism · Overlay")
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
            platform::transparent_overlay(editing);
            platform::overlay_visibility(editing || checking || platform::foreground_game());
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
            ui.heading("Rank in game");
            design::card().show(ui,|ui|{
                design::eyebrow(ui,"FUENTE DE DATOS PENDIENTE",design::PURPLE());ui.heading("Rango y elo reales");
                design::muted(ui,"El marcador final del juego puede mostrar MMR, pero todavía no tenemos una lectura automática fiable de ese dato.");
                design::muted(ui,"Esta función espera una fuente verificable. No se muestran rangos inventados ni estimados a partir de otra modalidad.");
            });
            return;
        }
        ui.heading("FPS y ping");
        design::muted(
            ui,
            "Un indicador pequeño, verde y sin fondo sobre Rocket League.",
        );
        ui.add_space(16.);
        design::card().show(ui,|ui|{
            design::eyebrow(ui,"FPS DEL JUEGO",design::GREEN());
            let before=self.overlay_preferences.fps;ui.checkbox(&mut self.overlay_preferences.fps,"Mostrar FPS");
            if before!=self.overlay_preferences.fps{self.telemetry.enable(self.overlay_preferences.fps);self.save_overlay();}
            let reading=self.telemetry.snapshot();design::muted(ui,if self.overlay_preferences.fps{if reading.live_fps().is_some(){"Midiendo fotogramas del juego".into()}else{reading.status.clone()}}else{"Desactivado".into()});
            ui.add_space(10.);draw_metrics(ui,reading.live_fps(),None,false);
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
        ui.add_space(12.);
        design::card().show(ui,|ui|{
            design::eyebrow(ui,"PING",design::MUTED());let mut enabled=false;ui.add_enabled(false,egui::Checkbox::new(&mut enabled,"Mostrar ping"));
            design::muted(ui,"Pendiente de una fuente verificable del ping de esta partida. No se sustituye por un ping a Internet ni por el de una región.");
            draw_ping_preview(ui);
        });
    }
    fn save_overlay(&mut self) {
        self.config["overlay"] = serde_json::to_value(&self.overlay_preferences).unwrap();
        if let Err(error) = platform::save_config(&self.root, &self.config) {
            self.status = error;
        }
    }
}
fn draw_ping_preview(ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        let (rect, _) = ui.allocate_exact_size(Vec2::new(22., 18.), egui::Sense::hover());
        for i in 0..4 {
            let h = 4. + 4. * i as f32;
            ui.painter().rect_filled(
                egui::Rect::from_min_size(
                    egui::pos2(rect.left() + i as f32 * 5., rect.bottom() - h),
                    Vec2::new(3., h),
                ),
                1,
                design::GREEN(),
            );
        }
        ui.label(RichText::new("—").color(design::GREEN()));
    });
}
