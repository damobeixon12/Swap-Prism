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
        self.status="Perfil añadido. Pulsa Aplicar abajo.".into();
    }
    fn profile_request(&mut self,settings:Value){
        self.queue_profile(settings);
    }

    pub(super) fn profile_editor(&mut self,ui:&mut egui::Ui){
        let mut changed=false;
        egui::ScrollArea::vertical().id_salt("profile-editor").show(ui,|ui|{
            let width=ui.available_width().min(830.);let banner_h=(width*100./420.).clamp(120.,165.);
            let(rect,_)=ui.allocate_exact_size(Vec2::new(width,banner_h+100.),egui::Sense::hover());
            let painter=ui.painter().clone();
            painter.rect_filled(rect,12,d::PANEL());
            let banner_rect=egui::Rect::from_min_size(rect.min,Vec2::new(width,banner_h));
            let banner=s(&self.profile_draft,"banner").to_owned();
            if banner.is_empty(){
                let mut mesh=egui::Mesh::default();let glow=d::GREEN().gamma_multiply(0.16);
                for(pos,color)in[(banner_rect.left_top(),glow),(banner_rect.right_top(),Color32::TRANSPARENT),(banner_rect.right_bottom(),Color32::TRANSPARENT),(banner_rect.left_bottom(),glow.gamma_multiply(0.5))]{mesh.colored_vertex(pos,color);}
                mesh.add_triangle(0,1,2);mesh.add_triangle(0,2,3);painter.add(mesh);
            }else{
                egui::Image::from_uri(format!("file://{banner}")).corner_radius(egui::CornerRadius{nw:12,ne:12,sw:0,se:0}).paint_at(ui,banner_rect);
            }
            painter.rect_stroke(rect,12,egui::Stroke::new(1_f32,d::LINE()),egui::StrokeKind::Inside);
            let button=egui::Rect::from_min_size(egui::pos2(banner_rect.right()-156.,banner_rect.top()+16.),Vec2::new(140.,34.));
            if ui.put(button,egui::Button::new(RichText::new("Cambiar banner").size(13.5)).wrap_mode(egui::TextWrapMode::Extend).fill(d::BACKGROUND())).clicked()&&!self.busy{self.pick_profile_image("banner");}
            let avatar_rect=egui::Rect::from_min_size(egui::pos2(rect.left()+32.,banner_rect.bottom()-38.),Vec2::splat(100.));
            painter.rect_filled(avatar_rect.expand(3.),16,d::PANEL());painter.rect_filled(avatar_rect,14,d::BACKGROUND());
            let avatar=s(&self.profile_draft,"avatar").to_owned();
            if avatar.is_empty(){
                let name=s(&self.profile_draft,"name");let initial=name.chars().next().or_else(||s(&self.snapshot,"player_name").chars().next()).unwrap_or('S').to_uppercase().to_string();
                painter.text(avatar_rect.center()-Vec2::new(0.,8.),egui::Align2::CENTER_CENTER,initial,egui::FontId::proportional(42.),d::TEXT());
            }else{
                egui::Image::from_uri(format!("file://{avatar}")).corner_radius(14).paint_at(ui,avatar_rect);
            }
            let strip=egui::Rect::from_min_max(egui::pos2(avatar_rect.left(),avatar_rect.bottom()-26.),avatar_rect.right_bottom());
            painter.rect_filled(strip,egui::CornerRadius{nw:0,ne:0,sw:14,se:14},Color32::from_black_alpha(185));
            painter.text(strip.center(),egui::Align2::CENTER_CENTER,"Cambiar",egui::FontId::proportional(13.),Color32::WHITE);
            painter.rect_stroke(avatar_rect,14,egui::Stroke::new(1_f32,d::LINE()),egui::StrokeKind::Inside);
            if ui.interact(avatar_rect,ui.id().with("profile-avatar"),egui::Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand).on_hover_text("Elegir foto de perfil").clicked()&&!self.busy{self.pick_profile_image("avatar");}
            let left=avatar_rect.right()+22.;let top=banner_rect.bottom()+8.;
            painter.text(egui::pos2(left,top),egui::Align2::LEFT_TOP,"Nombre visible",egui::FontId::proportional(13.),d::MUTED());
            let field=egui::Rect::from_min_max(egui::pos2(left,top+22.),egui::pos2(rect.right()-30.,top+66.));
            let mut name=s(&self.profile_draft,"name").to_owned();
            let edit=ui.put(field,egui::TextEdit::singleline(&mut name).hint_text(s(&self.snapshot,"player_name")).char_limit(32).font(egui::FontId::proportional(20.)).margin(Vec2::new(14.,10.)));
            if edit.changed(){self.profile_draft["name"]=json!(name);changed=true;}
            ui.add_space(18.);
            ui.horizontal_wrapped(|ui|{
                d::muted(ui,"PNG, JPG o WebP · banner 420:100");ui.add_space(8.);
                let link=|ui:&mut egui::Ui,text:&str,enabled:bool|ui.add_enabled(enabled,egui::Label::new(RichText::new(text).underline().color(d::MUTED())).sense(egui::Sense::click())).on_hover_cursor(egui::CursorIcon::PointingHand).clicked();
                if link(ui,"Quitar avatar",!self.busy&&!s(&self.profile_draft,"avatar").is_empty()){self.profile_draft["avatar"]=json!("");changed=true;}
                if link(ui,"Quitar banner",!self.busy&&!s(&self.profile_draft,"banner").is_empty()){self.profile_draft["banner"]=json!("");changed=true;}
                if self.profile_active()&&link(ui,"Volver al perfil original",!self.busy){self.profile_request(json!({"name":"","avatar":"","banner":"","title":null}));}
            });
            ui.add_space(6.);
            d::muted(ui,"Solo lo ves tú, en tu cliente. Los demás jugadores ven tu perfil original.");
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
            d::eyebrow(ui,"PERFIL LOCAL APLICADO",d::PURPLE());
            let settings=&self.profile_state["settings"];
            let name=s(settings,"name");
            ui.label(RichText::new(if name.is_empty(){"Nombre original".to_owned()}else{format!("Nombre visible: {name}")}).strong());
            d::muted(ui,format!("Avatar: {} · Banner: {}",if s(settings,"avatar").is_empty(){"original"}else{"personalizado"},if s(settings,"banner").is_empty(){"original"}else{"personalizado"}));
            if settings["title"].is_object(){ui.label(RichText::new(format!("Título: {} · sustituye {}",s(&settings["title"],"text"),s(&settings["title"],"target"))).color(d::GREEN()));}
            if settings["rank"].is_object(){ui.label(RichText::new(format!("Fake rank: {} · {} MMR · {}",ranks::tier_label(settings["rank"]["tier"].as_u64().unwrap_or(0)as usize),settings["rank"]["mmr"],ranks::mode_label(settings["rank"]["playlist"].as_i64().unwrap_or(11)))).color(d::GREEN()));}
            if self.profile_state["partial"]==true{ui.label(RichText::new("Aplicación parcial tras restaurar un paquete. Vuelve a aplicar el perfil para completarlo.").color(d::RED()));}
            ui.horizontal_wrapped(|ui|{
                if ui.button("Editar perfil").clicked(){self.section="Fake".into();self.fake_tab=1;}
                if ui.button("Editar título").clicked(){self.section="Fake".into();self.fake_tab=2;}
                ui.menu_button(RichText::new("Revertir un elemento…").color(d::RED()),|ui|{
                    for(key,label)in [("name","Nombre"),("avatar","Avatar"),("banner","Banner"),("title","Título"),("rank","Fake rank")]{
                        if ui.add_enabled(!self.busy,egui::Button::new(label)).clicked(){
                            let mut settings=self.profile_pending.clone().unwrap_or_else(||self.profile_state["settings"].clone());
                            settings[key]=if key=="title"||key=="rank"{Value::Null}else{json!("")};self.queue_profile(settings);ui.close_menu();
                        }
                    }
                });
                if ui.add_enabled(!self.busy&&self.profile_state["settings"]["title"].is_object(),d::button("Revertir título",d::RED())).clicked(){self.title_apply(Value::Null);}
                if ui.add_enabled(!self.busy,d::button("Revertir solo el perfil",d::RED())).clicked(){self.profile_request(json!({"name":"","avatar":"","banner":"","title":null}));}
            });
        });
        ui.add_space(12.);
    }
}
