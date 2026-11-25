use super::*;
use crate::design as d;

impl App {
    pub(super) fn go_to(&mut self, section: &str) {
        self.section = section.into();
        self.nav_expanded=section.into();
        if section=="Workshop" && self.workshop_data["maps"].as_array().map(|v|v.is_empty()).unwrap_or(true) && !self.busy{self.call("workshop_catalog",json!({"action":"workshop_catalog","refresh":true}));}
        if section == "Settings"
            || (section == "Skins" && !SLOTS.iter().any(|(slot, _)| *slot == self.view))
        {
            self.select_view("Wheels");
        }
    }
    pub fn render(&mut self, ctx: &egui::Context) {
        egui::SidePanel::left("main-navigation").exact_width(260.).resizable(false)
            .frame(egui::Frame::new().fill(Color32::from_rgb(23,25,29)).inner_margin(12)).show(ctx,|ui|{
                ui.horizontal(|ui|{ui.add(egui::Image::new(egui::include_image!("../assets/logo.png")).fit_to_exact_size(Vec2::splat(27.)).corner_radius(5));ui.label(RichText::new("Swap Prism").strong().size(15.));});
                ui.add_space(16.);d::search(ui,&mut self.navigation_search,"Buscar una sección…");ui.add_space(14.);
                egui::ScrollArea::vertical().id_salt("sidebar-nav").max_height((ui.available_height()-112.).max(150.)).show(ui,|ui|self.navigation(ui));
                ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT),|ui|{
                    if ui.add_enabled(!self.busy,egui::Button::new("Salir").min_size(Vec2::new(ui.available_width(),32.))).clicked(){ctx.send_viewport_cmd(egui::ViewportCommand::Close);}
                    d::muted(ui,"F8 · Mostrar / ocultar");d::muted(ui,format!("v{VERSION}"));
                });
            });
        egui::TopBottomPanel::bottom("status")
            .frame(
                egui::Frame::new()
                    .fill(d::BACKGROUND)
                    .inner_margin(egui::Margin::symmetric(24, 12)),
            )
            .show(ctx, |ui| {
                ui.horizontal_wrapped(|ui| {
                    if self.busy {
                        ui.spinner();
                    }
                    ui.label(RichText::new(&self.status).size(14.).color(d::MUTED));
                    if let Some(fraction)=self.job_fraction{ui.add(egui::ProgressBar::new(fraction).desired_width(150.).show_percentage());}
                });
            });
        egui::TopBottomPanel::top("header").frame(egui::Frame::new().fill(d::BACKGROUND).inner_margin(egui::Margin::symmetric(28,20))).show(ctx,|ui|{
            ui.horizontal(|ui|{
                ui.vertical(|ui|{d::eyebrow(ui,"Swap Prism / TU ESPACIO",d::MUTED);ui.heading(match self.section.as_str(){"Home"=>"Inicio","Skins"=>"Skins","Texturas"=>"Texturas","Fake"=>"Fake display","Overlay"=>"Overlay","Queue"=>"Cambios realizados","Studio"=>"Personalización","Workshop"=>"Workshop","Tracker"=>"Tracker","Community"=>"Discord",_=>"Ajustes"});});
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center),|ui|{
                    let dirty=self.studio_dirty();let count=self.plans.len()+usize::from(dirty)+usize::from(self.profile_pending.is_some());let can=count>0&&!self.busy&&!self.running&&self.game.is_some();
                    let apply=ui.add_enabled(can,d::button(&format!("Aplicar cambios ({})",count),d::GREEN));self.apply_anchor=apply.rect;
                    if apply.clicked(){self.call("apply",json!({"action":"apply","game":self.game,"version":self.game_version,"team":if self.team==0{"Blue"}else{"Orange"},"plans":self.plans.values().collect::<Vec<_>>(),"studio_settings":if dirty{self.studio_draft.clone()}else{Value::Null},"profile_settings":self.profile_pending.clone()}));}
                    let text=if self.running{"Juego abierto"}else{"Juego cerrado"};ui.label(RichText::new(text).size(14.).color(if self.running{d::MUTED}else{d::GREEN}));
                });
            });
            let queued:Vec<_>=self.plans.iter().map(|(k,v)|(k.clone(),s(v,"label").to_owned())).collect();
            if !queued.is_empty()||self.profile_pending.is_some()||self.studio_dirty(){
                ui.add_space(12.);let mut remove=None;
                egui::ScrollArea::horizontal().id_salt("top-pending").max_height(32.).show(ui,|ui|{ui.horizontal(|ui|{
                    d::muted(ui,"Pendiente:");
                    for(key,label)in queued{if ui.add(egui::Button::new(format!("{label}  ×"))).on_hover_text("Quitar de la cola").clicked(){remove=Some(key);}}
                    if self.profile_pending.is_some(){if ui.button("Perfil / título / rango · revisar").clicked(){self.section="Fake".into();self.fake_tab=1;}}
                    if self.studio_dirty(){if ui.button("Personalización · revisar").clicked(){self.section="Studio".into();}}
                });});
                if let Some(key)=remove{self.plans.remove(&key);self.save_queue();}
            }
        });
        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(d::BACKGROUND)
                    .inner_margin(egui::Margin::symmetric(26, 12)),
            )
            .show(ctx, |ui| {
                let page=format!("{}:{}:{}:{}:{}:{}:{}",self.section,self.view,self.texture_category,self.fake_tab,self.overlay_tab,self.studio_tab,self.studio_sound_kind);
                if self.page_motion_key!=page{self.page_motion_key=page;self.page_motion_start=Instant::now();}
                let t=(self.page_motion_start.elapsed().as_secs_f32()/0.22).clamp(0.,1.);let ease=1.-(1.-t).powi(3);
                if t<1.{ctx.request_repaint();}
                ui.multiply_opacity(0.55+ease*0.45);ui.add_space((1.-ease)*7.);
                match self.section.as_str() {
                "Home" => self.home(ui),
                "Skins" => self.skins(ui),
                "Texturas" => self.textures(ui),
                "Queue" => self.changes(ui),
                "Studio" => self.studio_editor(ui),
                "Workshop" => self.workshop_editor(ui),
                "Tracker" => self.tracker_editor(ui),
                "Community" => self.community(ui),
                "Fake" if self.fake_tab == 1 => self.profile_editor(ui),
                "Fake" if self.fake_tab == 2 => self.title_editor(ui),
                "Fake" => self.fake_rank_editor(ui),
                "Overlay" => {
                    egui::ScrollArea::vertical()
                        .id_salt("overlay-options")
                        .show(ui, |ui| self.overlay_controls(ui));
                }
                _ => self.settings(ui),
                }
            });
    }

    fn home(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical().id_salt("home-scroll").show(ui,|ui|{
            d::card().inner_margin(28).show(ui,|ui|{
                ui.horizontal(|ui|{
                    ui.add(egui::Image::new(egui::include_image!("../assets/logo.png")).fit_to_exact_size(Vec2::splat(82.)).corner_radius(10));
                    ui.add_space(12.);ui.vertical(|ui|{ui.label(RichText::new("Swap Prism").size(30.).strong());d::muted(ui,"Rocket League · Personalización local");ui.add_space(8.);d::muted(ui,if self.running{"Rocket League está abierto"}else{"Rocket League está cerrado"});});
                });
            });
            ui.add_space(20.);d::eyebrow(ui,"ACCESOS",d::MUTED);
            for(key,title,copy)in [("Skins","Inventario","Selecciona un objeto y cambia su apariencia"),("Texturas","Texturas","Pelota, boostmeter y decals"),("Studio","Personalización","Sonidos, colores y pantalla de inicio"),("Fake","Perfil y títulos","Nombre, avatar y títulos personalizados"),("Queue","Cambios realizados","Revisa lo que sigue aplicado y vuelve al original")]{
                let clicked=d::card().inner_margin(16).show(ui,|ui|{
                    ui.horizontal(|ui|{let(r,_)=ui.allocate_exact_size(Vec2::splat(22.),egui::Sense::hover());crate::icons::paint(ui,r,key,d::MUTED);ui.vertical(|ui|{ui.label(RichText::new(title).strong());d::muted(ui,copy);});ui.with_layout(egui::Layout::right_to_left(egui::Align::Center),|ui|ui.button("Abrir").clicked()).inner}).inner
                }).inner;
                if clicked{self.go_to(key);}ui.add_space(4.);
            }
        });
    }

    fn preview_at(&self, ui: &mut egui::Ui, rect: egui::Rect, row: &Value) {
        let url = s(row, "Image");
        let path=platform::thumbnail_path(&self.root,url);
        if !url.is_empty() && path.is_file() {
            egui::Image::from_uri(format!("file://{}", path.display()))
                .fit_to_exact_size(rect.size())
                .paint_at(ui, rect);
        } else {
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "Sin imagen",
                egui::FontId::proportional(12.),
                d::MUTED,
            );
        }
    }

    fn item_list(
        &self,
        ui: &mut egui::Ui,
        id: &str,
        rows: &[Value],
        selected: Option<u64>,
    ) -> Option<Value> {
        if rows.is_empty() {
            ui.add_space(20.);
            d::muted(ui, "No hay artículos para esta búsqueda.");
            return None;
        }
        let mut choice = None;
        let equipped = self.equipped(&self.view).and_then(|v| v["ID"].as_u64());
        egui::ScrollArea::vertical()
            .id_salt(id)
            .max_height(ui.available_height().max(120.))
            .show_rows(ui, 68., rows.len(), |ui, range| {
                for i in range {
                    let row = &rows[i];
                    let is_selected = selected == row["ID"].as_u64();
                    let (rect, response) = ui.allocate_exact_size(
                        Vec2::new(ui.available_width(), 62.),
                        egui::Sense::click(),
                    );
                    ui.painter().rect_filled(
                        rect,
                        11,
                        if is_selected {
                            Color32::from_rgb(34, 63, 79)
                        } else if response.hovered() {
                            d::FIELD
                        } else {
                            d::PANEL
                        },
                    );
                    self.preview_at(
                        ui,
                        egui::Rect::from_min_size(rect.min + Vec2::new(8., 7.), Vec2::splat(48.)),
                        row,
                    );
                    let label_rect = egui::Rect::from_min_max(
                        rect.min + Vec2::new(68., 9.),
                        rect.max - Vec2::new(8., 25.),
                    );
                    ui.painter()
                        .with_clip_rect(label_rect.intersect(ui.clip_rect()))
                        .text(
                            label_rect.left_top(),
                            egui::Align2::LEFT_TOP,
                            s(row, "Product"),
                            egui::FontId::proportional(15.),
                            d::TEXT,
                        );
                    let note = format!(
                        "#{}{}{}",
                        row["ID"],
                        if id == "owned" && equipped == row["ID"].as_u64() {
                            "  ·  Equipado"
                        } else {
                            ""
                        },
                        if s(row, "AssetPackage").is_empty() {
                            "  ·  Solo consulta"
                        } else {
                            ""
                        }
                    );
                    ui.painter().text(
                        rect.min + Vec2::new(68., 42.),
                        egui::Align2::LEFT_CENTER,
                        note,
                        egui::FontId::proportional(13.),
                        d::MUTED,
                    );
                    if response.clicked() {
                        choice = Some(row.clone());
                    }
                    if self.design_dir.is_none() {
                        response.on_hover_text(s(row, "Product"));
                    }
                }
            });
        choice
    }

    fn skins(&mut self, ui: &mut egui::Ui) {
        if ["Paint Finish", "Engine Audio", "Player Anthem"].contains(&self.view.as_str()) {
            d::muted(ui,"Esta categoría está disponible para consultar. Todavía no permite aplicar cambios.");
        }
        let title = self
            .donor
            .as_ref()
            .or(self.target.as_ref())
            .map(|v| s(v, "Product"))
            .unwrap_or_else(|| {
                SLOTS
                    .iter()
                    .find(|(k, _)| *k == self.view)
                    .map(|(_, n)| *n)
                    .unwrap_or("Skins")
            })
            .to_owned();
        ui.heading(title);
        d::muted(
            ui,
            "1. Tu objeto   →   2. Nueva apariencia   →   3. Añadir y aplicar",
        );
        ui.add_space(12.);
        d::card().inner_margin(14).show(ui,|ui|{
            ui.horizontal(|ui|{
                if let Some(row)=self.donor.as_ref().or(self.target.as_ref()){self.thumb(ui,row,58.);}
                ui.vertical(|ui|{d::eyebrow(ui,"EQUIPO",d::MUTED);egui::ComboBox::from_id_salt("team").width(92.).selected_text(if self.team==0{"Azul"}else{"Naranja"}).show_ui(ui,|ui|{if ui.selectable_value(&mut self.team,0,"Azul").changed(){self.target=None;}if ui.selectable_value(&mut self.team,1,"Naranja").changed(){self.target=None;}});});
                ui.vertical(|ui|{d::eyebrow(ui,"COLOR",d::MUTED);ui.add_enabled_ui(self.view!="Body",|ui|{egui::ComboBox::from_id_salt("color").width(110.).selected_text(&self.color).show_ui(ui,|ui|{for c in &self.colors{ui.selectable_value(&mut self.color,c.clone(),c);}});});});
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center),|ui|{
                    let supported=["Body","Decal","Wheels","Rocket Boost","Trail","Goal Explosion","Topper","Antenna","Player Banner","Avatar Border"].contains(&self.view.as_str());
                    let pair=self.target.as_ref().zip(self.donor.as_ref());let has_assets=pair.map(|(a,b)|!s(a,"AssetPackage").is_empty()&&!s(b,"AssetPackage").is_empty()&&(a["ID"]!=b["ID"]||self.color!="Original")).unwrap_or(false);
                    if ui.add_enabled(supported&&has_assets&&!self.busy,d::button("Añadir cambio",d::GREEN)).clicked(){let target=self.target.as_ref().unwrap();let donor=self.donor.as_ref().unwrap();self.queue_plan(s(target,"AssetPackage").into(),json!({"category":target["Slot"],"kind":"item","target_id":target["ID"],"donor_id":donor["ID"],"color":self.color,"label":format!("{} → {} · {}",s(target,"Product"),s(donor,"Product"),self.color)}));self.save_queue();self.status="Añadido. Puedes seguir eligiendo o aplicar los cambios de la cola.".into();}
                });
            });
        });
        ui.add_space(14.);
        if self.view == "Body" {
            ui.label(RichText::new("Se conserva la física del coche original. Solo se aceptan modelos visuales compatibles.").size(15.).color(d::MUTED));
        }
        let height = (ui.available_height() - 44.).max(180.);
        ui.columns(2, |columns| {
            d::card().inner_margin(14).show(&mut columns[0], |ui| {
                ui.set_min_height(height);
                d::eyebrow(ui, "01 / TU INVENTARIO", d::CYAN);
                d::search(ui, &mut self.owned_query, "Busca tu objeto por nombre o ID");
                let query = self.owned_query.to_lowercase();
                let rows: Vec<_> = self
                    .owned(&self.view)
                    .into_iter()
                    .filter(|r| {
                        format!("{} {}", s(r, "Product"), r["ID"])
                            .to_lowercase()
                            .contains(&query)
                    })
                    .collect();
                let selected = self.target.as_ref().and_then(|v| v["ID"].as_u64());
                if let Some(v) = self.item_list(ui, "owned", &rows, selected) {
                    self.target = Some(v);
                }
            });
            d::card().inner_margin(14).show(&mut columns[1], |ui| {
                ui.set_min_height(height);
                d::eyebrow(ui, "02 / NUEVA APARIENCIA", d::PURPLE);
                d::search(ui, &mut self.query, "Busca una apariencia por nombre o ID");
                let query = self.query.to_lowercase();
                let rows: Vec<_> = self
                    .rows
                    .iter()
                    .filter(|r| {
                        s(r, "Slot") == self.view
                            && format!("{} {}", s(r, "Product"), r["ID"])
                                .to_lowercase()
                                .contains(&query)
                    })
                    .cloned()
                    .collect();
                let selected = self.donor.as_ref().and_then(|v| v["ID"].as_u64());
                if let Some(v) = self.item_list(ui, "donors", &rows, selected) {
                    self.donor = Some(v);
                }
            });
        });
    }
}

impl App {
    fn textures(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui|{ui.selectable_value(&mut self.texture_shop,false,"Mi biblioteca");ui.selectable_value(&mut self.texture_shop,true,"Cosméticos · Descargar");});
        if self.texture_shop{self.media_panel(ui,false);return;}
        ui.heading(if self.selected_texture_name.is_empty() {
            if self.texture_category == "Decal" {
                "Decals"
            } else {
                &self.texture_category
            }
        } else {
            &self.selected_texture_name
        });
        d::muted(
            ui,
            "Elige un diseño y pulsa Añadir. Después aplica la cola con Rocket League cerrado.",
        );
        ui.add_space(14.);
        ui.horizontal_wrapped(|ui| {
            if ui
                .add_enabled(!self.busy, egui::Button::new("Importar pack"))
                .clicked()
            {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("Texturas", &["zip", "json", "png", "dds"])
                    .pick_file()
                {
                    self.call("import", json!({"action":"import","path":path}));
                }
            }
            if ui
                .add_enabled(!self.busy, egui::Button::new("Elegir carpeta"))
                .clicked()
            {
                if let Some(path) = rfd::FileDialog::new().pick_folder() {
                    self.call("import", json!({"action":"import","path":path}));
                }
            }
            if self.texture_category == "Decal" {
                egui::ComboBox::from_id_salt("texture-target")
                    .width(215.)
                    .selected_text(
                        self.texture_target
                            .as_ref()
                            .map(|v| s(v, "Product"))
                            .unwrap_or("Elige tu decal original"),
                    )
                    .show_ui(ui, |ui| {
                        for row in self.owned("Decal") {
                            if ui
                                .selectable_label(
                                    self.texture_target.as_ref().map(|v| &v["ID"])
                                        == Some(&row["ID"]),
                                    s(&row, "Product"),
                                )
                                .clicked()
                            {
                                self.texture_target = Some(row);
                            }
                        }
                    });
            }
        });
        ui.add_space(6.);
        d::search(ui, &mut self.pack_query, "Buscar en esta categoría");
        let query = self.pack_query.to_lowercase();
        let mut packs: Vec<_> = self
            .packs
            .iter()
            .filter(|p| {
                s(p, "category") == self.texture_category
                    && s(p, "name").to_lowercase().contains(&query)
            })
            .cloned()
            .collect();
        packs.sort_by_key(|p| s(p, "name").to_lowercase());
        if packs.is_empty() {
            d::card().show(ui,|ui|{ui.heading("Tu biblioteca empieza aquí");d::muted(ui,"Importa tus texturas para verlas en esta categoría. Puedes usar un ZIP o la configuración JSON del pack.");});
            return;
        }
        let count = d::columns(ui.available_width());
        egui::ScrollArea::vertical().id_salt("texture-grid").show(ui,|ui|{
            for chunk in packs.chunks(count){ui.columns(count,|columns|{for(i,pack)in chunk.iter().enumerate(){d::card().inner_margin(14).show(&mut columns[i],|ui|{
                let width=ui.available_width();let rect=ui.allocate_exact_size(Vec2::new(width,126.),egui::Sense::hover()).0;
                ui.painter().rect_filled(rect,12,d::BACKGROUND);
                ui.put(rect.shrink(8.),egui::Image::from_uri(format!("file://{}",s(pack,"preview"))).max_size(Vec2::new(width-16.,110.)));
                let title=ui.add_sized([width,40.],egui::Label::new(RichText::new(s(pack,"name")).strong().size(15.)).wrap());
                if title.clicked(){self.selected_texture_name=s(pack,"name").into();}
                let enabled=!self.busy&&(self.texture_category!="Decal"||self.texture_target.is_some());
                if ui.add_enabled(enabled,d::button("Añadir",d::GREEN).min_size(Vec2::new(width,38.))).clicked(){
                    let target=self.texture_target.clone().unwrap_or(Value::Null);let key=if self.texture_category=="Decal"{s(&target,"AssetPackage").to_owned()}else if self.texture_category=="Pelota"{"GameInfo_Soccar_SF.upk".into()}else{"GFX_Hud_SF.upk".into()};
                    self.queue_plan(key,json!({"category":pack["category"],"kind":"texture","digest":pack["digest"],"target_id":target["ID"],"label":format!("{} · {}",s(pack,"category"),s(pack,"name"))}));self.selected_texture_name=s(pack,"name").into();self.save_queue();self.status="Textura añadida. Aplica los cambios cuando el juego esté cerrado.".into();
                }
            });}});ui.add_space(6.);}
        });
    }

    fn changes(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui|{ui.checkbox(&mut self.changes_history,"Mostrar historial restaurado");if ui.add_enabled(!self.busy,egui::Button::new("Comprobar archivos")).clicked(){self.call("refresh_state",json!({"action":"refresh_state"}));}});
        let installed = self
            .state
            .as_object()
            .map(|m| m.values().filter(|v| s(v, "status") == "installed" && s(v,"observed_status")!="original" && s(&v["target_item"],"Slot")!="Profile" && v["hud_display_companion"]!=true).count())
            .unwrap_or(0) + usize::from(self.profile_active());
        d::muted(
            ui,
            format!(
                "{} pendientes  ·  {} instalados",
                self.plans.len()+usize::from(self.studio_dirty())+usize::from(self.profile_pending.is_some()),
                installed
            ),
        );
        ui.add_space(14.);
        if self.profile_pending.is_some(){d::card().show(ui,|ui|{
            ui.label(RichText::new("Perfil, título o rango pendientes").strong());d::muted(ui,"Se aplican junto con los demás cambios, desde el botón superior.");
            if ui.button("Revisar perfil").clicked(){self.section="Fake".into();self.fake_tab=1;}
            if ui.add(d::button("Descartar selección",d::RED)).clicked(){self.profile_pending=None;self.profile_draft=self.profile_state["settings"].clone();self.save_profile_draft();let _=fs::write(self.root.join("data/profile-queue.json"),b"{}");}
        });}
        self.profile_change_card(ui);
        if self.studio_dirty(){d::card().show(ui,|ui|{ui.label(RichText::new("Personalización pendiente").strong());d::muted(ui,"Sonidos, colores o inicio. Se incluyen en Aplicar cambios.");if ui.button("Revisar").clicked(){self.section="Studio".into();}if ui.add(d::button("Descartar pendientes",d::RED)).clicked(){self.studio_draft=self.studio_applied.clone();self.save_studio_draft();}});}

        let mut remove = None;
        let mut restore = None;
        egui::ScrollArea::vertical().id_salt("changes-list").max_height((ui.available_height()-78.).max(100.)).show(ui,|ui|{
            if !self.plans.is_empty(){d::eyebrow(ui,"PENDIENTES DE APLICAR",d::GREEN);ui.add_space(4.);}
            for(key,plan)in &self.plans{d::card().inner_margin(16).show(ui,|ui|{ui.horizontal(|ui|{
                ui.vertical(|ui|{d::eyebrow(ui,"PENDIENTE",d::GREEN);ui.label(RichText::new(s(plan,"label")).strong());});
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center),|ui|{if ui.add_enabled(!self.busy,d::button("Quitar",d::RED)).clicked(){remove=Some(key.clone());}});
            });});ui.add_space(4.);}
            if let Some(entries)=self.state.as_object(){if !entries.is_empty(){ui.add_space(10.);d::eyebrow(ui,"CAMBIOS REGISTRADOS",d::CYAN);ui.add_space(4.);}for(key,entry)in entries{
                if s(&entry["target_item"],"Slot")=="Profile"||entry["hud_display_companion"]==true{continue;}
                let inactive=s(entry,"status").starts_with("restored")||s(entry,"observed_status")=="original";if inactive&&!self.changes_history{continue;}
                let(status,color)=if inactive{("Restaurado",d::MUTED)}else if s(entry,"observed_status")=="external"{("Modificado fuera de Swap Prism",d::MUTED)}else{d::status(entry)};d::card().inner_margin(16).show(ui,|ui|{
                    ui.horizontal(|ui|{
                        self.thumb(ui,&entry["donor_item"],48.);
                        let width=(ui.available_width()-120.).max(100.);ui.allocate_ui_with_layout(Vec2::new(width,0.),egui::Layout::top_down(egui::Align::LEFT),|ui|{
                            ui.set_min_width(width);d::eyebrow(ui,status,color);ui.label(RichText::new(format!("{} → {}",s(&entry["target_item"],"Product"),s(&entry["donor_item"],"Product"))).strong());if let Some(paint)=entry["color"]["name"].as_str(){d::muted(ui,paint);}
                        });
                        let restored=inactive;if ui.add_enabled(!restored&&!self.busy&&!self.running,d::button("Revertir",d::RED)).clicked(){restore=Some(key.clone());}
                    });
                });ui.add_space(4.);
            }}
            if self.plans.is_empty()&&self.state.as_object().map(|v|v.is_empty()).unwrap_or(true){d::card().show(ui,|ui|{ui.heading("Todavía no has hecho cambios");d::muted(ui,"Elige una apariencia en Skins o añade un diseño de Texturas. Aquí podrás revisarlo y volver al original.");});}
        });
        if let Some(key) = remove {
            self.plans.remove(&key);
            self.save_queue();
            self.status = "Selección quitada de la cola.".into();
        }
        if let Some(key) = restore {
            self.call("restore", json!({"action":"restore","names":[key]}));
        }
        ui.add_space(14.);
        ui.horizontal_wrapped(|ui| {
            if ui
                .add_enabled(
                    !self.busy && !self.running && installed > 0,
                    d::button("Restaurar todos los originales", d::RED),
                )
                .clicked()
            {
                self.call("restore", json!({"action":"restore"}));
            }
            d::muted(
                ui,
                if self.running {
                    "Cierra Rocket League para restaurar."
                } else {
                    "Tus copias originales se conservan."
                },
            );
        });
    }

    pub(super) fn pending_feature(&mut self, ui: &mut egui::Ui) {
        let overlay = self.section == "Overlay";
        let title = if overlay {
            d::OVERLAY_TABS[self.overlay_tab]
        } else {
            d::FAKE_TABS[self.fake_tab]
        };
        ui.heading(title);
        ui.add_space(18.);
        d::card().inner_margin(28).show(ui,|ui|{
            d::eyebrow(ui,"PRÓXIMAMENTE",if overlay{d::CYAN}else{d::PURPLE});ui.add_space(18.);
            ui.label(RichText::new(match(overlay,if overlay{self.overlay_tab}else{self.fake_tab}){
                (true,0)=>"FPS y ping, sin perder de vista la partida.",(true,1)=>"El rango y el elo junto a los jugadores.",(true,_)=>"Un espacio para lo que viene.",
                (false,0)=>"Tu rango visual, a tu manera.",(false,1)=>"Tu foto y tu nombre, en el mismo lugar.",_=>"Un título con tu palabra y tu color."
            }).size(23.).strong());ui.add_space(12.);
            d::muted(ui,if overlay{"Esta función todavía no está disponible. Aquí podrás configurar la información que ves durante la partida."}else{"Esta función todavía no está disponible. Aquí podrás personalizar cómo se ve tu perfil en tu cliente."});ui.add_space(18.);
            ui.add_enabled(false,egui::Button::new("En preparación"));
        });
    }

    fn settings(&mut self, ui: &mut egui::Ui) {
        if self.view == "Inventory" {
            ui.horizontal(|ui| {
                ui.heading("Todo tu inventario");
                if ui.button("Volver a ajustes").clicked() {
                    self.view = "Wheels".into();
                }
            });
            d::search(ui, &mut self.query, "Buscar por nombre o ID");
            let query = self.query.to_lowercase();
            let rows: Vec<_> = self.snapshot["inventory"]
                .as_array()
                .into_iter()
                .flatten()
                .filter(|v| {
                    format!("{} {}", s(v, "Product"), v["ID"])
                        .to_lowercase()
                        .contains(&query)
                })
                .cloned()
                .collect();
            self.item_list(ui, "inventory", &rows, None);
            return;
        }
        egui::ScrollArea::vertical().id_salt("settings-scroll").show(ui,|ui|{
            d::card().show(ui,|ui|{
                d::eyebrow(ui,"ROCKET LEAGUE",d::CYAN);ui.heading("Tu juego, conectado");ui.add_space(10.);
                if let Some(path)=&self.game{d::muted(ui,path.display().to_string());}else{d::muted(ui,"No se ha encontrado el juego. Selecciona la carpeta rocketleague, que contiene Binaries y TAGame.");}
                if ui.button("Elegir carpeta del juego").clicked(){if let Some(path)=rfd::FileDialog::new().set_title("Elige rocketleague (contiene Binaries y TAGame)").pick_folder(){if platform::valid_game(&path){self.game=Some(path.clone());self.config["game_path"]=json!(path);self.status=platform::save_config(&self.root,&self.config).err().unwrap_or("Ruta guardada.".into());}else{self.status="Elige la carpeta rocketleague que contiene Binaries y TAGame.".into();}}}
            });ui.add_space(12.);
            d::card().show(ui,|ui|{
                d::eyebrow(ui,"TU CUENTA",d::PURPLE);ui.heading("Inventario local");d::muted(ui,"Se lee el último guardado de Rocket League.");ui.add_space(10.);
                ui.horizontal_wrapped(|ui|{if ui.add_enabled(!self.busy,egui::Button::new("Refrescar inventario")).clicked(){self.call("bootstrap",json!({"action":"bootstrap"}));}if ui.button("Ver inventario completo").clicked(){self.view="Inventory".into();self.query.clear();}});
            });ui.add_space(12.);
            d::card().show(ui,|ui|{
                d::eyebrow(ui,"Swap Prism",d::GREEN);ui.heading(format!("Versión {VERSION}"));
                d::muted(ui,"Canal estable: damobeixon12/Swap-Prism · GitHub Releases");if s(&self.config,"update_manifest_url").is_empty(){d::muted(ui,"El canal todavía no está configurado.");}else if ui.button("Buscar actualizaciones").clicked(){self.check_updates_ui();}
                ui.add_space(12.);d::muted(ui,"F8 muestra u oculta la app. Ctrl+Q sale.");d::muted(ui,"Cierra Rocket League antes de aplicar o restaurar cambios.");
            });
        });
    }
}

