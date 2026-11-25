use super::*;
use crate::design as d;

impl App {
    pub(super) fn save_profile_draft(&self) {
        let path=self.root.join("data/profile-draft.json");
        if let Some(parent)=path.parent(){let _=fs::create_dir_all(parent);}
        if let Ok(bytes)=serde_json::to_vec_pretty(&self.profile_draft){let _=fs::write(path,bytes);}
    }

    pub(super) fn queue_profile(&mut self,mut settings:Value){
        if settings.get("rank").is_none(){settings["rank"]=self.profile_pending.as_ref().unwrap_or(&self.profile_state["settings"])["rank"].clone();}
        if settings.get("title").is_none(){settings["title"]=self.profile_pending.as_ref().unwrap_or(&self.profile_state["settings"])["title"].clone();}
        self.profile_draft=settings.clone();self.save_profile_draft();
        self.profile_pending=if settings==self.profile_state["settings"]{None}else{Some(settings)};
        let _=fs::write(self.root.join("data/profile-queue.json"),serde_json::to_vec_pretty(self.profile_pending.as_ref().unwrap_or(&json!({}))).unwrap());
        self.status="Perfil añadido a los cambios pendientes de arriba.".into();
    }
    fn profile_request(&mut self,settings:Value){
        self.queue_profile(settings);
    }

    pub(super) fn profile_editor(&mut self,ui:&mut egui::Ui){
        ui.heading("Tu perfil, a tu manera");
        d::muted(ui,"Nombre, avatar y banner que ves en tu cliente.");
        ui.add_space(18.);
        let mut changed=false;
        egui::ScrollArea::vertical().id_salt("profile-editor").show(ui,|ui|{
            d::card().inner_margin(24).show(ui,|ui|{
                d::eyebrow(ui,"VISTA PREVIA · PERFIL LOCAL",d::PURPLE);
                ui.add_space(12.);
                let banner=s(&self.profile_draft,"banner").to_owned();
                if !banner.is_empty(){ui.add(egui::Image::from_uri(format!("file://{banner}")).fit_to_exact_size(Vec2::new(ui.available_width().min(520.),110.)).corner_radius(14));}
                ui.horizontal(|ui|{
                    let avatar=s(&self.profile_draft,"avatar").to_owned();
                    let response=if avatar.is_empty(){
                        ui.add(egui::Image::new(egui::include_image!("../assets/logo.png")).fit_to_exact_size(Vec2::splat(88.)).corner_radius(18).sense(egui::Sense::click()))
                    }else{ui.add(egui::Image::from_uri(format!("file://{avatar}")).fit_to_exact_size(Vec2::splat(88.)).corner_radius(18).sense(egui::Sense::click()))};
                    if response.on_hover_text("Elegir foto de perfil").clicked()&&!self.busy{self.pick_profile_image("avatar");}
                    ui.vertical(|ui|{
                        ui.label(RichText::new("NOMBRE VISIBLE").size(11.).color(d::MUTED));
                        let mut name=s(&self.profile_draft,"name").to_owned();
                        let response=ui.add(egui::TextEdit::singleline(&mut name).hint_text(s(&self.snapshot,"player_name")).char_limit(32).font(egui::FontId::proportional(24.)).desired_width(ui.available_width()-16.));
                        if response.changed(){self.profile_draft["name"]=json!(name);changed=true;}
                        d::muted(ui,"Pulsa el avatar o escribe tu nombre. Vacío conserva el original.");
                    });
                });
            });
            ui.add_space(14.);
            d::card().inner_margin(20).show(ui,|ui|{
                ui.label(RichText::new("Imágenes del perfil").strong());
                ui.horizontal_wrapped(|ui|{
                    if ui.add_enabled(!self.busy,egui::Button::new("Elegir avatar…")).clicked(){self.pick_profile_image("avatar");}
                    if ui.add_enabled(!self.busy,egui::Button::new("Elegir banner…")).clicked(){self.pick_profile_image("banner");}
                    if ui.add_enabled(!self.busy&&!s(&self.profile_draft,"avatar").is_empty(),d::button("Quitar avatar",d::RED)).clicked(){self.profile_draft["avatar"]=json!("");changed=true;}
                    if ui.add_enabled(!self.busy&&!s(&self.profile_draft,"banner").is_empty(),d::button("Quitar banner",d::RED)).clicked(){self.profile_draft["banner"]=json!("");changed=true;}
                });
                d::muted(ui,"PNG, JPG o WebP. Avatar cuadrado; banner recortado a 420:100.");
                ui.add_space(6.);
                d::muted(ui,"El avatar personalizado se limita a tu jugador; los rivales conservan el original. El marco se conserva. El banner es un recurso compartido; los banners equipados de Skins se cambian desde su categoría.");
            });
            ui.add_space(14.);
            d::card().inner_margin(20).show(ui,|ui|{
                d::eyebrow(ui,"APLICACIÓN CON EL JUEGO CERRADO",d::GREEN);
                d::muted(ui,"El nombre se prepara para el menú, HUD y marcador. La cobertura dentro del juego todavía necesita una prueba visual; los nombres sobre los coches pueden usar otra fuente.");
                ui.horizontal_wrapped(|ui|{
                    if ui.add_enabled(!self.busy&&self.game.is_some(),d::button("Añadir perfil a cambios",d::GREEN)).clicked(){self.save_profile_draft();self.profile_request(self.profile_draft.clone());}
                    if ui.add_enabled(!self.busy&&self.profile_active(),d::button("Revertir perfil",d::RED)).clicked(){self.profile_request(json!({"name":"","avatar":"","banner":"","title":null}));}
                });
                if self.running{ui.label(RichText::new("Cierra Rocket League para aplicar o revertir el perfil.").color(d::RED));}
            });
        });
        if changed{self.queue_profile(self.profile_draft.clone());}
    }

    fn pick_profile_image(&mut self,kind:&str){
        if let Some(path)=rfd::FileDialog::new().add_filter("Imagen",&["png","jpg","jpeg","webp"]).pick_file(){
            self.call("profile_import",json!({"action":"profile_import","kind":kind,"path":path}));
        }
    }

    pub(super) fn profile_active(&self)->bool{self.profile_state["packages"].as_object().map(|p|!p.is_empty()).unwrap_or(false)}

    pub(super) fn profile_change_card(&mut self,ui:&mut egui::Ui){
        if !self.profile_active(){return;}
        d::card().inner_margin(18).show(ui,|ui|{
            d::eyebrow(ui,"PERFIL LOCAL APLICADO",d::PURPLE);
            let settings=&self.profile_state["settings"];
            let name=s(settings,"name");
            ui.label(RichText::new(if name.is_empty(){"Nombre original".to_owned()}else{format!("Nombre visible: {name}")}).strong());
            d::muted(ui,format!("Avatar: {} · Banner: {}",if s(settings,"avatar").is_empty(){"original"}else{"personalizado"},if s(settings,"banner").is_empty(){"original"}else{"personalizado"}));
            if settings["title"].is_object(){ui.label(RichText::new(format!("Título: {} · sustituye {}",s(&settings["title"],"text"),s(&settings["title"],"target"))).color(d::GREEN));}
            if settings["rank"].is_object(){ui.label(RichText::new(format!("Fake rank: {} · {} MMR · {}",ranks::tier_label(settings["rank"]["tier"].as_u64().unwrap_or(0)as usize),settings["rank"]["mmr"],ranks::mode_label(settings["rank"]["playlist"].as_i64().unwrap_or(11)))).color(d::GREEN));}
            if self.profile_state["partial"]==true{ui.label(RichText::new("Aplicación parcial tras restaurar un paquete. Vuelve a aplicar el perfil para completarlo.").color(d::RED));}
            ui.horizontal_wrapped(|ui|{
                if ui.button("Editar perfil").clicked(){self.section="Fake".into();self.fake_tab=1;}
                if ui.button("Editar título").clicked(){self.section="Fake".into();self.fake_tab=2;}
                ui.menu_button(RichText::new("Revertir un elemento…").color(d::RED),|ui|{
                    for(key,label)in [("name","Nombre"),("avatar","Avatar"),("banner","Banner"),("title","Título"),("rank","Fake rank")]{
                        if ui.add_enabled(!self.busy,egui::Button::new(label)).clicked(){
                            let mut settings=self.profile_pending.clone().unwrap_or_else(||self.profile_state["settings"].clone());
                            settings[key]=if key=="title"||key=="rank"{Value::Null}else{json!("")};self.queue_profile(settings);ui.close_menu();
                        }
                    }
                });
                if ui.add_enabled(!self.busy&&self.profile_state["settings"]["title"].is_object(),d::button("Revertir título",d::RED)).clicked(){self.title_apply(Value::Null);}
                if ui.add_enabled(!self.busy,d::button("Revertir solo el perfil",d::RED)).clicked(){self.profile_request(json!({"name":"","avatar":"","banner":"","title":null}));}
            });
        });
        ui.add_space(12.);
    }
}
