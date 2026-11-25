//! Development-only rendering check. Uses egui's own framebuffer screenshot event.
//! Never clicks Apply, queues a selection, changes settings, or writes game files.
use super::*;
const CASES: [(&str, &str, &str, usize); 42] = [
    ("home", "Home", "Wheels", 0),
    ("tracker", "Tracker", "", 0),
    ("discord", "Community", "", 0),
    ("fake-rank-small", "Fake", "", 0),
    ("workshop", "Workshop", "", 0),
    ("workshop-small", "Workshop", "", 0),
    ("skins-wheels", "Skins", "Wheels", 0),
    ("skins-body", "Skins", "Body", 0),
    ("skins-boost", "Skins", "Rocket Boost", 0),
    ("skins-decal", "Skins", "Decal", 0),
    ("textures-meter", "Texturas", "Boostmeter", 0),
    ("textures-ball", "Texturas", "Pelota", 0),
    ("textures-decals", "Texturas", "Decal", 0),
    ("fake-rank", "Fake", "", 0),
    ("fake-profile", "Fake", "", 1),
    ("fake-titles", "Fake", "", 2),
    ("overlay-fps", "Overlay", "", 0),
    ("overlay-rank", "Overlay", "", 1),
    ("overlay-soon", "Overlay", "", 2),
    ("changes", "Queue", "", 0),
    ("settings", "Settings", "Wheels", 0),
    ("inventory", "Settings", "Inventory", 0),
    ("home-ball", "Home", "Wheels", 1),
    ("home-meter", "Home", "Wheels", 2),
    ("home-next", "Home", "Wheels", 3),
    ("home-small", "Home", "Wheels", 0),
    ("skins-small", "Skins", "Wheels", 0),
    ("textures-small", "Texturas", "Boostmeter", 0),
    ("fake-titles-multicolor", "Fake", "", 2),
    ("fake-titles-small", "Fake", "", 2),
    ("fake-titles-long", "Fake", "", 2),
    ("studio-palette", "Studio", "", 0),
    ("studio-sounds", "Studio", "", 1),
    ("studio-post", "Studio", "", 4),
    ("sound-library", "Studio", "", 5),
    ("sound-downloads", "Studio", "", 6),
    ("texture-downloads-alpha", "Texturas", "Boostmeter", 0),
    ("texture-downloads-vgm", "Texturas", "Boostmeter", 1),
    ("studio-reset", "Studio", "", 2),
    ("studio-season", "Studio", "", 3),
    ("studio-small", "Studio", "", 0),
    ("title-library", "Fake", "", 2),
];
impl App {
    pub fn prepare_design(&mut self, ctx: &egui::Context) {
        if self.design_dir.is_none()
            || self.busy
            || !self.snapshot["inventory"].is_array()
            || self.design_wait != 0
            || self.design_index >= CASES.len()
        {
            return;
        }
        let (name, section, view, index) = CASES[self.design_index];
        ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(
            if name.ends_with("-small") {
                Vec2::new(1060., 740.)
            } else {
                Vec2::new(1280., 860.)
            },
        ));
        self.section = section.into();
        self.texture_shop=name.starts_with("texture-downloads");self.texture_source=if name=="texture-downloads-vgm"{1}else{0};
        self.sound_browser=0;
        self.query.clear();
        self.owned_query.clear();
        self.pack_query.clear();
        if section == "Skins" {
            self.select_view(view);
            self.target = self
                .equipped(view)
                .or_else(|| self.owned(view).into_iter().next());
            self.donor = self
                .rows
                .iter()
                .find(|r| {
                    s(r, "Slot") == view
                        && if view == "Rocket Boost" {
                            r["ID"] == 32
                        } else {
                            self.target
                                .as_ref()
                                .map(|t| r["ID"] != t["ID"])
                                .unwrap_or(true)
                        }
                })
                .cloned();
        } else if section == "Texturas" {
            self.texture_category = view.into();
            self.texture_target = None;
            self.selected_texture_name.clear();
        } else if section == "Fake" {
            self.fake_tab = index;
            self.title_page=if name=="title-library"{0}else{1};
            if index==2{
                self.title_draft["multicolor"]=json!(name=="fake-titles-multicolor");
                self.title_draft["text"]=json!(if name=="fake-titles-long"{"MI TÍTULO PERSONALIZADO CON EL TEXTO QUE YO QUIERA"}else{"SWAP PRISM ON TOP"});
            }
        } else if section == "Studio" {
            self.studio_tab=if name=="studio-post"||index>=5{1}else{index};
            self.sound_browser=if index==5{1}else if index==6{2}else{0};
            self.studio_sound_kind=if name=="studio-post"{"post"}else{"reset"}.into();
        } else if section == "Overlay" {
            self.overlay_tab = index;
        } else if section == "Settings" {
            self.view = view.into();
        } else if section == "Home" {
            self.feature_page = index;
            self.feature_changed = Instant::now();
        }
        self.design_wait = 1;
    }
    pub fn capture_design(&mut self, ctx: &egui::Context) {
        let Some(dir) = self.design_dir.clone() else {
            return;
        };
        if self.design_wait == 0 {
            return;
        }
        if self.design_requested {
            let screenshots = ctx.input(|i| {
                i.events
                    .iter()
                    .filter_map(|e| {
                        if let egui::Event::Screenshot { image, .. } = e {
                            Some(image.clone())
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>()
            });
            if let Some(image) = screenshots.first() {
                let _ = fs::create_dir_all(&dir);
                let bytes: Vec<_> = image.pixels.iter().flat_map(|p| p.to_array()).collect();
                let name = CASES[self.design_index].0;
                let _ = image::save_buffer(
                    dir.join(format!("{name}.png")),
                    &bytes,
                    image.size[0] as u32,
                    image.size[1] as u32,
                    image::ColorType::Rgba8,
                );
                self.design_index += 1;
                self.design_wait = 0;
                self.design_requested = false;
                if self.design_index == CASES.len() {
                    let _=fs::write(dir.join("report.json"),json!({"version":VERSION,"rendered_screens":CASES.iter().map(|c|c.0).collect::<Vec<_>>(),"game_writes":false,"catalog":self.rows.len(),"inventory":self.snapshot["inventory"].as_array().map(|v|v.len()),"packs":self.packs.len()}).to_string());
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
        } else {
            self.design_wait += 1;
            if self.design_wait >= 8 {
                self.design_requested = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
            }
        }
    }
}
