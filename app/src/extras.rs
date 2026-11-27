//! Extras: reapply after a game update, apply when the game closes, saved sets (and sharing them),
//! Ctrl+K search and favourites.
use super::*;
use crate::design as d;

/// Studio settings that make sense on another PC (no local files).
const SHARED_STUDIO: [&str; 3] = ["palette", "reset_color", "reset_enabled"];

/// The game package a queued plan writes (its key in swaps.json).
fn package(key: &str, plan: &Value) -> String {
    if s(plan, "kind").starts_with("workshop") { "Labs_Utopia_P.upk".into() } else { key.to_owned() }
}

/// Queue key of a texture plan (same rule as the Texturas page).
fn texture_key(category: &str, target: &Value) -> String {
    match category { "Pelota" => "GameInfo_Soccar_SF.upk".into(), "Boostmeter" => "GFX_Hud_SF.upk".into(), _ => s(target, "AssetPackage").to_owned() }
}

/// Small five-point star, filled or outlined.
pub(super) fn star(painter: &egui::Painter, center: egui::Pos2, radius: f32, filled: bool, color: Color32) {
    let point = |k: usize, r: f32| { let a = -std::f32::consts::FRAC_PI_2 + k as f32 * std::f32::consts::PI / 5.; center + Vec2::new(a.cos(), a.sin()) * r };
    let points: Vec<egui::Pos2> = (0..10).map(|k| point(k, if k % 2 == 0 { radius } else { radius * 0.45 })).collect();
    if filled {
        // Five convex kites (centre, inner, tip, inner): egui only fills convex shapes.
        for k in 0..5 { painter.add(egui::Shape::convex_polygon(vec![center, points[(2 * k + 9) % 10], points[2 * k], points[2 * k + 1]], color, egui::Stroke::NONE)); }
    } else {
        painter.add(egui::Shape::closed_line(points, egui::Stroke::new(1.4_f32, color)));
    }
}

impl App {
    // ---------- favourites ----------
    pub(super) fn is_favorite(&self, key: &str) -> bool { self.config["favorites"].as_array().is_some_and(|a| a.iter().any(|v| v == key)) }

    pub(super) fn toggle_favorite(&mut self, key: &str) {
        let mut list: Vec<Value> = self.config["favorites"].as_array().cloned().unwrap_or_default();
        if let Some(i) = list.iter().position(|v| v == key) { list.remove(i); } else { list.push(json!(key)); }
        self.config["favorites"] = json!(list);
        if let Err(e) = platform::save_config(&self.root, &self.config) { self.status = e; }
    }

    /// Clickable star at `center`; drawn when it is a favourite or `show` (hover).
    pub(super) fn favorite_star(&mut self, ui: &mut egui::Ui, center: egui::Pos2, key: &str, show: bool) {
        let on = self.is_favorite(key);
        let response = ui.interact(egui::Rect::from_center_size(center, Vec2::splat(24.)), egui::Id::new(("favorite", key)), egui::Sense::click());
        if !(on || show || response.hovered()) { return; }
        ui.painter().circle_filled(center, 12., d::BACKGROUND().gamma_multiply(0.8));
        star(ui.painter(), center, 7.5, on, if on { d::GREEN() } else { d::MUTED() });
        if response.on_hover_cursor(egui::CursorIcon::PointingHand).on_hover_text(if on { "Quitar de favoritos" } else { "Añadir a favoritos" }).clicked() { self.toggle_favorite(key); }
    }

    // ---------- what is applied ----------
    /// Plans currently installed: recorded at apply time, restored ones drop out.
    pub(super) fn applied_plans(&self) -> BTreeMap<String, Value> {
        let saved: BTreeMap<String, Value> = serde_json::from_value(read_json(&self.root.join("data/applied.json"))).unwrap_or_default();
        saved.into_iter().filter(|(k, p)| self.state[package(k, p).as_str()]["status"] == "installed").collect()
    }

    fn record_applied(&mut self) {
        let mut all = self.applied_plans();
        for (k, p) in std::mem::take(&mut self.applying) {
            if s(&p, "kind") == "workshop_restore" { all.remove(&k); } else { all.insert(k, p); }
        }
        let _ = fs::write(self.root.join("data/applied.json"), serde_json::to_vec_pretty(&all).unwrap());
    }

    /// Called with every engine result for «apply».
    pub(super) fn after_apply(&mut self, ctx: &egui::Context, ok: bool) {
        if ok { self.record_applied(); } else { self.applying.clear(); }
        if std::mem::take(&mut self.notify_apply) {
            ctx.send_viewport_cmd(egui::ViewportCommand::RequestUserAttention(egui::UserAttentionType::Informational));
            if ok { self.status = format!("Listo: cambios aplicados al cerrar el juego. {}", self.status); }
        }
    }

    /// Changes the game put back to the original (an update or Epic's Verify): installed by us, original on disk.
    pub(super) fn reset_by_game(&self) -> Vec<String> {
        let layers = &self.profile_state["packages"];
        self.state.as_object().into_iter().flatten()
            .filter(|(name, e)| e["status"] == "installed" && e["observed_status"] == "original" && e["hud_display_companion"] != true
                && s(&e["target_item"], "Slot") != "Profile" && layers[name.as_str()]["base_sha256"] != e["original_sha256"])
            .map(|(n, _)| n.clone()).collect()
    }

    /// The request that installs the queue (the «Aplicar» button).
    pub(super) fn queue_request(&mut self) -> Value {
        self.applying = self.plans.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
        json!({"action":"apply","game":self.game,"version":self.game_version,"team":if self.team==0{"Blue"}else{"Orange"},
            "plans":self.plans.values().collect::<Vec<_>>(),"studio_settings":if self.studio_dirty(){self.studio_draft.clone()}else{Value::Null},
            "profile_settings":self.profile_pending.clone()})
    }

    /// The request that puts back what the game reset; None when nothing recorded can be reapplied.
    fn reapply_request(&mut self) -> Option<Value> {
        let reset = self.reset_by_game();
        let plans: Vec<(String, Value)> = self.applied_plans().into_iter().filter(|(k, p)| reset.contains(&package(k, p))).collect();
        let studio = reset.iter().any(|n| s(&self.state[n.as_str()]["target_item"], "Slot") == "Personalización") && self.studio_applied.as_object().is_some_and(|o| !o.is_empty());
        if plans.is_empty() && !studio { return None; }
        let request = json!({"action":"apply","game":self.game,"version":self.game_version,"team":"Blue","plans":plans.iter().map(|p|&p.1).collect::<Vec<_>>(),
            "studio_settings":if studio{self.studio_applied.clone()}else{Value::Null},
            "profile_settings":if self.profile_active(){self.profile_state["settings"].clone()}else{Value::Null}});
        self.applying = plans;
        Some(request)
    }

    fn start_reapply(&mut self) {
        match self.reapply_request() {
            Some(request) => self.call("apply", request),
            None => { self.reset_dismissed = true; self.status = "Esos cambios se aplicaron con una versión anterior de Swap Prism. Vuelve a añadirlos desde Skins o Texturas.".into(); }
        }
    }

    /// Runs the waiting apply a few seconds after the game closes (so its files are free).
    pub(super) fn apply_when_closed(&mut self, ctx: &egui::Context) {
        let Some(kind) = self.apply_waiting else { return };
        if kind == "queue" && self.pending_count() == 0 { self.apply_waiting = None; return; }
        if self.running || self.busy { self.closed_at = None; return; }
        if self.closed_at.get_or_insert_with(Instant::now).elapsed() < Duration::from_secs(4) { ctx.request_repaint_after(Duration::from_millis(500)); return; }
        self.apply_waiting = None;
        self.closed_at = None;
        self.notify_apply = true;
        if kind == "reapply" { self.start_reapply(); } else { let request = self.queue_request(); self.call("apply", request); }
    }

    /// Floating notice when the game reset some of your changes.
    pub(super) fn reset_bar(&mut self, ctx: &egui::Context) {
        if self.reset_dismissed || self.state.as_object().is_none() { return; }
        let count = self.reset_by_game().len();
        if count == 0 { return; }
        egui::Area::new(egui::Id::new("reset-bar")).anchor(egui::Align2::CENTER_TOP, Vec2::new(125., 18.)).order(egui::Order::Foreground).show(ctx, |ui| {
            egui::Frame::new().fill(d::PANEL()).stroke(egui::Stroke::new(1_f32, d::LINE())).corner_radius(12).inner_margin(egui::Margin { left: 20, right: 10, top: 8, bottom: 8 })
                .shadow(egui::Shadow { offset: [0, 8], blur: 24, spread: 0, color: Color32::from_black_alpha(110) }).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(if count == 1 { "Rocket League ha quitado 1 cambio".to_owned() } else { format!("Rocket League ha quitado {count} cambios") }).size(15.));
                    ui.add_space(14.);
                    if self.apply_waiting == Some("reapply") {
                        d::muted(ui, "Se pondrán al cerrar el juego");
                        if ui.add(egui::Button::new(RichText::new("Cancelar").color(d::MUTED())).frame(false)).clicked() { self.apply_waiting = None; }
                        return;
                    }
                    if ui.add(egui::Button::new(RichText::new("Ahora no").color(d::MUTED())).frame(false)).clicked() { self.reset_dismissed = true; }
                    let label = if self.running { "Ponerlos al cerrar el juego" } else { "Volver a ponerlos" };
                    if ui.add_enabled(!self.busy && self.game.is_some() && self.apply_waiting.is_none(), d::primary(label)).clicked() {
                        if self.running { self.apply_waiting = Some("reapply"); self.status = "Se volverán a poner cuando cierres Rocket League.".into(); } else { self.start_reapply(); }
                    }
                });
            });
        });
    }

    // ---------- sets ----------
    fn write_sets(&mut self) {
        let _ = fs::create_dir_all(self.root.join("data"));
        if let Err(e) = fs::write(self.root.join("data/sets.json"), serde_json::to_vec_pretty(&self.sets).unwrap()) { self.status = e.to_string(); }
    }

    fn save_set(&mut self) {
        let name = self.set_name.trim().to_owned();
        let mut plans = self.applied_plans();
        plans.extend(self.plans.clone());
        let profile = match &self.profile_pending { Some(p) => p.clone(), None if self.profile_active() => self.profile_state["settings"].clone(), None => Value::Null };
        self.sets.retain(|x| s(x, "name") != name);
        self.sets.push(json!({"name": name, "plans": plans, "studio": self.studio_draft, "profile": profile}));
        self.write_sets();
        self.set_name.clear();
        self.status = format!("Conjunto «{name}» guardado.");
    }

    /// Puts a set in the queue (only what differs from what is already applied).
    fn use_set(&mut self, set: &Value) {
        let applied = self.applied_plans();
        for (k, p) in set["plans"].as_object().into_iter().flatten() {
            if applied.get(k) != Some(p) { self.queue_plan(k.clone(), p.clone()); }
        }
        self.save_queue();
        if let Some(studio) = set["studio"].as_object() {
            for (k, v) in studio { self.studio_draft[k] = v.clone(); }
            self.save_studio_draft();
        }
        if set["profile"].is_object() { self.queue_profile(set["profile"].clone()); }
        self.status = if self.pending_count() == 0 { format!("«{}» ya es lo que tienes puesto.", s(set, "name")) } else { format!("«{}» listo. Pulsa Aplicar.", s(set, "name")) };
    }

    /// Shareable file: skins, textures, maps and colours. Never the profile or local sounds.
    fn export_set(&mut self, set: &Value) {
        let Some(path) = rfd::FileDialog::new().set_file_name(format!("{}.swapprism", s(set, "name"))).add_filter("Swap Prism", &["swapprism"]).save_file() else { return };
        let plans: serde_json::Map<String, Value> = set["plans"].as_object().into_iter().flatten()
            .filter(|(_, p)| ["item", "texture", "workshop"].contains(&s(p, "kind"))).map(|(k, p)| (k.clone(), p.clone())).collect();
        let studio: serde_json::Map<String, Value> = set["studio"].as_object().into_iter().flatten()
            .filter(|(k, _)| SHARED_STUDIO.contains(&k.as_str())).map(|(k, v)| (k.clone(), v.clone())).collect();
        let file = json!({"swapprism_set": 1, "name": set["name"], "plans": plans, "studio": studio});
        self.status = match fs::write(&path, serde_json::to_vec_pretty(&file).unwrap()) { Ok(()) => "Conjunto exportado. Compártelo con quien quieras.".into(), Err(e) => e.to_string() };
    }

    /// The original object for an imported plan: theirs if you own it, else yours in the same slot.
    fn own_target(&self, id: &Value) -> Option<Value> {
        let slot = self.rows.iter().find(|r| !id.is_null() && r["ID"] == *id).map(|r| s(r, "Slot").to_owned())?;
        let owned = self.owned(&slot);
        owned.iter().find(|r| r["ID"] == *id).cloned().or_else(|| self.equipped(&slot)).or_else(|| owned.first().cloned())
    }

    /// Reads a shared file. Everything is checked again here and by the engine when applying.
    fn import_set(&mut self) {
        let Some(path) = rfd::FileDialog::new().add_filter("Swap Prism", &["swapprism", "json"]).pick_file() else { return };
        let file = read_json(&path);
        if file["swapprism_set"] != 1 { self.status = "Ese archivo no es un conjunto de Swap Prism.".into(); return; }
        let (mut plans, mut skipped) = (serde_json::Map::new(), 0);
        for plan in file["plans"].as_object().into_iter().flatten().map(|(_, p)| p) {
            let kind = s(plan, "kind");
            let adapted = match kind {
                "item" => self.own_target(&plan["target_id"]).zip(self.rows.iter().find(|r| r["ID"] == plan["donor_id"]).cloned())
                    .filter(|(t, d)| !s(t, "AssetPackage").is_empty() && !s(d, "AssetPackage").is_empty() && s(t, "Slot") == s(d, "Slot"))
                    .map(|(t, d)| {
                        let color = if self.colors.iter().any(|c| c == s(plan, "color")) { s(plan, "color").to_owned() } else { "Original".into() };
                        (s(&t, "AssetPackage").to_owned(), json!({"category": t["Slot"], "kind": "item", "target_id": t["ID"], "donor_id": d["ID"], "color": color,
                            "label": format!("{} → {} · {}", s(&t, "Product"), s(&d, "Product"), color)}))
                    }),
                "texture" => self.packs.iter().find(|p| p["digest"] == plan["digest"]).cloned().and_then(|pack| {
                    let category = s(&pack, "category").to_owned();
                    let target = if category == "Decal" { self.own_target(&plan["target_id"])? } else { Value::Null };
                    Some((texture_key(&category, &target), json!({"category": category, "kind": "texture", "digest": pack["digest"], "target_id": target["ID"],
                        "label": format!("{} · {}", category, s(&pack, "name"))})))
                }),
                "workshop" => self.workshop_data["maps"].as_array().into_iter().flatten().find(|m| m["key"] == plan["map_key"] && m["local"].is_object())
                    .map(|m| ("workshop".to_owned(), json!({"kind": "workshop", "category": "Workshop", "target": "Labs_Utopia_P.upk", "map_key": m["key"], "label": format!("Mapa → {}", s(m, "name"))}))),
                _ => None,
            };
            match adapted { Some((k, p)) => { plans.insert(k, p); } None => skipped += 1 }
        }
        // Colours only, with the same shape as yours.
        let mut studio = serde_json::Map::new();
        for key in SHARED_STUDIO {
            let (theirs, mine) = (&file["studio"][key], &self.studio_draft[key]);
            if !theirs.is_null() && std::mem::discriminant(theirs) == std::mem::discriminant(mine) { studio.insert(key.into(), theirs.clone()); }
        }
        let base = s(&file, "name").chars().filter(|c| !c.is_control()).take(40).collect::<String>();
        let base = if base.trim().is_empty() { "Importado".to_owned() } else { base.trim().to_owned() };
        let mut name = base.clone();
        let mut n = 2;
        while self.sets.iter().any(|x| s(x, "name") == name) { name = format!("{base} ({n})"); n += 1; }
        self.sets.push(json!({"name": name, "plans": plans, "studio": studio, "profile": Value::Null}));
        self.write_sets();
        self.status = if skipped == 0 { format!("«{name}» importado. Pulsa Poner para usarlo.") } else { format!("«{name}» importado. {skipped} cambio(s) no se pueden usar en tu PC (falta la textura, el mapa o el objeto).") };
    }

    pub(super) fn sets_card(&mut self, ui: &mut egui::Ui) {
        let mut action: Option<(&str, Value)> = None;
        d::card().show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("Conjuntos").strong().size(16.));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.add_enabled(!self.busy, d::button("Importar", d::TEXT()).min_size(Vec2::new(0., 32.))).clicked() { action = Some(("import", Value::Null)); }
                });
            });
            ui.add_space(4.);
            for set in &self.sets {
                ui.horizontal(|ui| {
                    ui.set_min_height(36.);
                    ui.label(RichText::new(s(set, "name")).size(15.));
                    let count = set["plans"].as_object().map_or(0, |p| p.len());
                    d::muted(ui, if count == 1 { "1 cambio".to_owned() } else { format!("{count} cambios") });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.add(egui::Label::new(RichText::new("✕").color(d::MUTED())).sense(egui::Sense::click())).on_hover_text("Borrar").on_hover_cursor(egui::CursorIcon::PointingHand).clicked() { action = Some(("delete", set.clone())); }
                        ui.add_space(6.);
                        if ui.add(egui::Button::new(RichText::new("Exportar").color(d::MUTED())).frame(false)).clicked() { action = Some(("export", set.clone())); }
                        if ui.add_enabled(!self.busy, d::button("Poner", d::GREEN()).min_size(Vec2::new(0., 32.))).clicked() { action = Some(("use", set.clone())); }
                    });
                });
            }
            ui.add_space(6.);
            ui.horizontal(|ui| {
                ui.add(egui::TextEdit::singleline(&mut self.set_name).hint_text("Nombre, p. ej. Competitivo").desired_width(240.).margin(Vec2::new(10., 8.)));
                if ui.add_enabled(!self.set_name.trim().is_empty(), d::button("Guardar lo que tengo", d::TEXT()).min_size(Vec2::new(0., 34.))).clicked() { action = Some(("save", Value::Null)); }
            });
        });
        match action {
            Some(("import", _)) => self.import_set(),
            Some(("save", _)) => self.save_set(),
            Some(("use", set)) => self.use_set(&set),
            Some(("export", set)) => self.export_set(&set),
            Some(("delete", set)) => { self.sets.retain(|x| x["name"] != set["name"]); self.write_sets(); }
            _ => {}
        }
    }

    // ---------- Ctrl+K ----------
    /// Everything searchable: sections, skins, textures, sounds and maps. Favourites first; empty query shows favourites.
    fn search_all(&self, query: &str) -> Vec<(&'static str, String, String, Value)> {
        let q = query.trim().to_lowercase();
        let hit = |key: &str, name: &str| if q.is_empty() { self.is_favorite(key) } else { name.to_lowercase().contains(&q) };
        let mut out = Vec::new();
        for pack in &self.packs { if hit(&format!("pack:{}", s(pack, "digest")), s(pack, "name")) { out.push(("pack", s(pack, "name").to_owned(), format!("Textura · {}", s(pack, "category")), pack.clone())); } }
        for audio in &self.audio_library { if hit(&format!("sound:{}", s(audio, "path")), s(audio, "name")) { out.push(("sound", s(audio, "name").to_owned(), "Sonido".into(), audio.clone())); } }
        for map in self.workshop_data["maps"].as_array().into_iter().flatten() { if hit(&format!("map:{}", s(map, "key")), s(map, "name")) { out.push(("map", s(map, "name").to_owned(), "Mapa".into(), map.clone())); } }
        let mut seen = std::collections::HashSet::new();
        for row in &self.rows {
            let Some((_, slot)) = SLOTS.iter().find(|(k, _)| *k == s(row, "Slot")) else { continue };
            if s(row, "AssetPackage").is_empty() || !hit(&format!("item:{}", row["ID"]), s(row, "Product")) || !seen.insert((s(row, "Slot"), s(row, "Product"))) { continue; }
            out.push(("item", s(row, "Product").to_owned(), slot.to_string(), row.clone()));
            if out.len() > 200 { break; }
        }
        let key = |r: &(&str, String, String, Value)| match r.0 { "pack" => format!("pack:{}", s(&r.3, "digest")), "sound" => format!("sound:{}", s(&r.3, "path")), "map" => format!("map:{}", s(&r.3, "key")), _ => format!("item:{}", r.3["ID"]) };
        out.sort_by_key(|r| (!self.is_favorite(&key(r)), !r.1.to_lowercase().starts_with(&q)));
        let sections = crate::navigation::NAV.iter().filter(|(_, l)| !q.is_empty() && l.to_lowercase().contains(&q)).map(|(k, l)| ("section", l.to_string(), "Sección".to_owned(), json!(k)));
        let mut all: Vec<_> = sections.collect();
        all.extend(out);
        all.truncate(8);
        all
    }

    fn open_result(&mut self, kind: &str, value: &Value) {
        match kind {
            "section" => self.go_to(value.as_str().unwrap_or("Home")),
            "item" => { self.go_to("Skins"); self.select_view(s(value, "Slot")); self.query = s(value, "Product").into(); self.donor = Some(value.clone()); }
            "pack" => { self.go_to("Texturas"); self.texture_shop = false; self.texture_category = s(value, "category").into(); self.texture_target = None; self.pack_query = s(value, "name").into(); self.selected_texture_name = s(value, "name").into(); }
            "sound" => { self.go_to("Studio"); self.studio_tab = 1; self.studio_sound_kind = s(value, "kind").into(); self.sound_browser = 1; }
            _ => { self.go_to("Workshop"); self.workshop_query = s(value, "name").into(); }
        }
    }

    /// Ctrl+K: one box that finds anything and takes you there.
    pub(super) fn search_palette(&mut self, ctx: &egui::Context) {
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, egui::Key::K)) { self.palette = if self.palette.is_some() { None } else { Some(String::new()) }; self.palette_index = 0; }
        // Dev check: SP_SHOT_PALETTE=<query> opens the box in --screenshot mode.
        if self.shot.is_some() && self.palette.is_none() { self.palette = std::env::var("SP_SHOT_PALETTE").ok(); }
        let Some(mut query) = self.palette.take() else { return };
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) { return; }
        let results = self.search_all(&query);
        let (down, up, enter) = ctx.input_mut(|i| (i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown), i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp), i.key_pressed(egui::Key::Enter)));
        if down { self.palette_index += 1; }
        if up { self.palette_index = self.palette_index.saturating_sub(1); }
        self.palette_index = self.palette_index.min(results.len().saturating_sub(1));
        let (mut pick, mut close) = (if enter && !results.is_empty() { Some(self.palette_index) } else { None }, false);
        egui::Area::new(egui::Id::new("palette-backdrop")).order(egui::Order::Foreground).fixed_pos(egui::Pos2::ZERO).show(ctx, |ui| {
            let screen = ctx.screen_rect();
            ui.painter().rect_filled(screen, 0, Color32::from_black_alpha(150));
            if ui.allocate_rect(screen, egui::Sense::click()).clicked() { close = true; }
        });
        egui::Area::new(egui::Id::new("palette")).order(egui::Order::Tooltip).anchor(egui::Align2::CENTER_TOP, Vec2::new(125., 120.)).show(ctx, |ui| {
            egui::Frame::new().fill(d::PANEL()).stroke(egui::Stroke::new(1_f32, d::LINE())).corner_radius(14).inner_margin(egui::Margin::symmetric(8, 8))
                .shadow(egui::Shadow { offset: [0, 12], blur: 40, spread: 0, color: Color32::from_black_alpha(140) }).show(ui, |ui| {
                ui.set_width(580.);
                let edit = ui.add(egui::TextEdit::singleline(&mut query).hint_text("Busca una skin, una textura, un sonido, un mapa…").font(egui::FontId::proportional(17.)).frame(false).margin(Vec2::new(12., 12.)).desired_width(f32::INFINITY));
                edit.request_focus();
                if edit.changed() { self.palette_index = 0; }
                if results.is_empty() {
                    ui.add_space(4.);
                    ui.horizontal(|ui| { ui.add_space(12.); d::muted(ui, if query.trim().is_empty() { "Escribe para buscar. Tus favoritos ★ aparecen aquí." } else { "Sin resultados." }); });
                    ui.add_space(8.);
                    return;
                }
                ui.add_space(2.);
                for (i, (_, label, detail, _)) in results.iter().enumerate() {
                    let (rect, response) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 40.), egui::Sense::click());
                    if response.hovered() && ui.input(|i| i.pointer.delta() != Vec2::ZERO) { self.palette_index = i; }
                    if i == self.palette_index { ui.painter().rect_filled(rect, 9, d::LINE().gamma_multiply(0.7)); }
                    ui.painter().text(rect.left_center() + Vec2::new(14., 0.), egui::Align2::LEFT_CENTER, label, egui::FontId::proportional(15.), d::TEXT());
                    ui.painter().text(rect.right_center() - Vec2::new(14., 0.), egui::Align2::RIGHT_CENTER, detail, egui::FontId::proportional(13.), d::MUTED());
                    if response.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() { pick = Some(i); }
                }
            });
        });
        if let Some(i) = pick { let (kind, _, _, value) = &results[i]; self.open_result(kind, value); return; }
        if !close { self.palette = Some(query); }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn plan_packages() {
        assert_eq!(super::package("workshop", &serde_json::json!({"kind": "workshop_restore"})), "Labs_Utopia_P.upk");
        assert_eq!(super::package("Wheel_A_SF.upk", &serde_json::json!({"kind": "item"})), "Wheel_A_SF.upk");
        assert_eq!(super::texture_key("Pelota", &serde_json::Value::Null), "GameInfo_Soccar_SF.upk");
    }
}
