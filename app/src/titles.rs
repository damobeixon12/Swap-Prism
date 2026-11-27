use super::*;
use crate::design as d;

fn rgb(value:&str)->Color32{
    u32::from_str_radix(value.trim_start_matches('#'),16).map(|n|Color32::from_rgb((n>>16) as u8,(n>>8) as u8,n as u8)).unwrap_or(d::GREEN())
}
fn hex(c:Color32)->String{format!("#{:02X}{:02X}{:02X}",c.r(),c.g(),c.b())}

impl App {
    pub(super) fn title_editor(&mut self,ui:&mut egui::Ui){
        ui.horizontal_wrapped(|ui|{for(i,label)in [crate::i18n::t("Títulos"),crate::i18n::t("Personalizados"),crate::i18n::t("Título activo")].iter().enumerate(){if ui.selectable_label(self.title_page==i,*label).clicked(){self.title_page=i;}}});
        ui.add_space(14.);
        if self.title_page==1 {self.title_custom_editor(ui);return;}
        if self.title_page==2 {
            ui.heading(crate::i18n::t("Tu título activo"));
            let current=self.profile_state["settings"]["title"].clone();
            d::card().show(ui,|ui|{
                if current.is_object(){
                    ui.label(RichText::new(s(&current,"text")).size(24.).strong().color(rgb(s(&current,"color"))));
                    d::muted(ui,crate::i18n::tr(&format!("Reemplaza: {}",s(&current,"target"))));
                    if ui.button(crate::i18n::t("Editar este título")).clicked(){self.title_draft=current.clone();self.title_page=1;}
                    if ui.add_enabled(!self.busy,d::button(crate::i18n::t("Revertir título"),d::RED())).clicked(){self.title_apply(Value::Null);}
                }else{d::muted(ui,crate::i18n::t("Estás usando tu título original."));}
            });return;
        }
        ui.heading(crate::i18n::t("Estilos de títulos"));
        d::muted(ui,crate::i18n::t("Elige el color y el resplandor. Los textos de estas tarjetas son ejemplos de acabado, no tu inventario."));
        ui.add_space(12.);d::search(ui,&mut self.title_query,crate::i18n::t("Buscar un estilo…"));
        let styles=[("DEVELOPER","#00F0BB","#00CFA0"),("PSYONIX HACKATHON WINNER","#00F0BB","#00CFA0"),("RIVAL SERIES ANALYST","#00F0BB","#00CFA0"),("RLCS ANALYST","#00F0BB","#00CFA0"),("SLEEPER BUILD","#00F0BB","#00CFA0"),("STYLE ICON","#00F0BB","#00CFA0"),("S23 2V2 GLOBAL #1","#FFD282","#D29240"),("S23 3V3 GLOBAL #1","#FFD282","#D29240"),("S23 2V2 TOP 10","#FFE4F3","#E5ACD0"),("S23 2V2 TOP 25","#FFE4F3","#E5ACD0"),("GRAND CHAMPION","#FF3030","#FF0000"),("SUPERSONIC LEGEND","#FFFFFF","#FFFFFF")];
        let query=self.title_query.to_lowercase();let filtered:Vec<_>=styles.into_iter().filter(|v|v.0.to_lowercase().contains(&query)).collect();
        egui::ScrollArea::vertical().id_salt("title-styles").show(ui,|ui|{
            for row in filtered.chunks(2){ui.columns(2,|columns|{for(i,(label,color,glow))in row.iter().enumerate(){
                let width=columns[i].available_width();let(r,response)=columns[i].allocate_exact_size(Vec2::new(width,100.),egui::Sense::click());let painter=columns[i].painter();
                let selected=s(&self.title_draft,"style")==*label;
                painter.rect_filled(r,5,if selected{d::FIELD()}else{d::PANEL()});painter.rect_stroke(r,5,egui::Stroke::new(1_f32,if selected{d::MUTED()}else{d::FIELD()}),egui::StrokeKind::Inside);
                let c=rgb(color);let mut font=egui::FontId::proportional(17.);let g=painter.layout_no_wrap(label.to_string(),font.clone(),c);font.size*=((width-24.)/g.size().x.max(1.)).min(1.);
                for offset in [Vec2::new(1.,0.),Vec2::new(-1.,0.),Vec2::new(0.,1.),Vec2::new(0.,-1.)]{painter.text(r.center()+offset,egui::Align2::CENTER_CENTER,*label,font.clone(),rgb(glow).gamma_multiply(0.24));}
                painter.text(r.center(),egui::Align2::CENTER_CENTER,*label,font,c);
                if response.clicked(){
                    self.title_draft["style"]=json!(label);self.title_draft["color"]=json!(color);self.title_draft["glow"]=json!(glow);self.title_draft["glow_enabled"]=json!(true);self.title_draft["multicolor"]=json!(false);
                    if s(&self.title_draft,"target").is_empty(){if let Some(t)=self.snapshot["titles"].as_array().and_then(|a|a.iter().find(|t|t["equipped"]==true)){self.title_draft["target"]=t["id"].clone();}}
                    if !s(&self.title_draft,"text").is_empty()&&!s(&self.title_draft,"target").is_empty(){self.title_apply(self.title_draft.clone());}
                    let _=fs::write(self.root.join("data/title-draft.json"),serde_json::to_vec_pretty(&self.title_draft).unwrap());self.title_page=1;
                }
            }});ui.add_space(10.);}
        });
    }
    pub(super) fn title_apply(&mut self,title:Value){
        let mut settings=self.profile_pending.clone().unwrap_or_else(||self.profile_state["settings"].clone());
        if !settings.is_object(){settings=json!({"name":"","avatar":"","banner":""});}
        settings["title"]=title;
        self.queue_profile(settings);
    }
    pub(super) fn title_custom_editor(&mut self,ui:&mut egui::Ui){
        ui.heading(crate::i18n::t("Títulos personalizados"));
        d::muted(ui,crate::i18n::t("Tu palabra, tu color y tu resplandor. Un swap visual sobre uno de tus títulos."));
        ui.add_space(16.);
        let mut changed=false;
        egui::ScrollArea::vertical().id_salt("title-editor").max_height((ui.available_height()-64.).max(120.)).show(ui,|ui|{
            d::card().inner_margin(24).show(ui,|ui|{
                d::eyebrow(ui,crate::i18n::t("VISTA PREVIA ORIENTATIVA"),d::PURPLE());
                let text=s(&self.title_draft,"text").to_uppercase();
                let color=rgb(s(&self.title_draft,"color"));let color2=rgb(s(&self.title_draft,"color2"));let glow=rgb(s(&self.title_draft,"glow"));
                let (rect,_)=ui.allocate_exact_size(Vec2::new(ui.available_width(),90.),egui::Sense::hover());
                ui.painter().rect_filled(rect,14,Color32::from_rgb(12,29,51));
                let avatar_rect=egui::Rect::from_min_size(rect.min+Vec2::new(18.,16.),Vec2::splat(58.));
                let avatar=s(&self.profile_state["settings"],"avatar");
                if avatar.is_empty(){egui::Image::new(egui::include_image!("../assets/logo.png")).corner_radius(12).paint_at(ui,avatar_rect);}else{egui::Image::from_uri(format!("file://{avatar}")).corner_radius(12).paint_at(ui,avatar_rect);}
                let alias=s(&self.profile_state["settings"],"name");let player=if alias.is_empty(){s(&self.snapshot,"player_name")}else{alias};
                ui.painter().text(rect.min+Vec2::new(96.,20.),egui::Align2::LEFT_TOP,player,egui::FontId::proportional(21.),d::TEXT());
                let title=if text.is_empty(){crate::i18n::t("TU TÍTULO")}else{&text};
                let mut font=egui::FontId::proportional(18.);
                let mut galley=ui.painter().layout_no_wrap(title.to_owned(),font.clone(),color);
                if galley.size().x>rect.width()-128.{font.size*=((rect.width()-128.)/galley.size().x).max(0.30);galley=ui.painter().layout_no_wrap(title.to_owned(),font.clone(),color);}
                let pos=rect.min+Vec2::new(96.,49.);
                if self.title_draft["glow_enabled"]!=false{
                    for (radius,alpha) in [(1.,8)]{for (x,y) in [(radius,0.),(-radius,0.),(0.,radius),(0.,-radius)]{ui.painter().galley_with_override_text_color(pos+Vec2::new(x,y),galley.clone(),Color32::from_rgba_unmultiplied(glow.r(),glow.g(),glow.b(),alpha));}}
                }
                if self.title_draft["multicolor"]==true{
                    let chars:Vec<_>=title.chars().collect();let mut job=egui::text::LayoutJob::default();
                    for(i,ch)in chars.iter().enumerate(){let t=i as f32/(chars.len().saturating_sub(1).max(1) as f32);let c=Color32::from_rgb((color.r() as f32+(color2.r() as f32-color.r() as f32)*t)as u8,(color.g() as f32+(color2.g() as f32-color.g() as f32)*t)as u8,(color.b() as f32+(color2.b() as f32-color.b() as f32)*t)as u8);job.append(&ch.to_string(),0.,egui::TextFormat{font_id:font.clone(),color:c,..Default::default()});}
                    let g=ui.fonts(|f|f.layout_job(job));ui.painter().galley(pos,g,Color32::WHITE);
                }else{ui.painter().galley(pos,galley,color);}
            });
            ui.add_space(12.);
            d::card().inner_margin(20).show(ui,|ui|{
                ui.label(RichText::new(crate::i18n::t("Escribe lo que quieras")).strong());
                let mut text=s(&self.title_draft,"text").to_owned();
                if ui.add(egui::TextEdit::singleline(&mut text).char_limit(48).hint_text(crate::i18n::t("Ejemplo: SWAP PRISM ON TOP")).desired_width(ui.available_width())).changed(){self.title_draft["text"]=json!(text);changed=true;}
                ui.horizontal_wrapped(|ui|{
                    for(label,color,glow)in [(crate::i18n::t("Verde"),"#75EAA6","#75EAA6"),(crate::i18n::t("Rojo GC"),"#FF3030","#FF0000"),(crate::i18n::t("Blanco SSL"),"#FFFFFF","#FFFFFF"),(crate::i18n::t("Dorado"),"#FFD45A","#EAA52B"),(crate::i18n::t("Cian"),"#68DAF9","#25BFE8")]{if ui.button(label).clicked(){self.title_draft["color"]=json!(color);self.title_draft["glow"]=json!(glow);self.title_draft["glow_enabled"]=json!(true);changed=true;}}
                });
                ui.horizontal_wrapped(|ui|{
                    for(key,label)in [("color","Color"),("color2",crate::i18n::t("Segundo color")),("glow",crate::i18n::t("Resplandor"))]{let c=rgb(s(&self.title_draft,key));let mut channels=[c.r(),c.g(),c.b()];ui.label(label);if ui.color_edit_button_srgb(&mut channels).changed(){self.title_draft[key]=json!(hex(Color32::from_rgb(channels[0],channels[1],channels[2])));changed=true;}}
                });
                ui.horizontal_wrapped(|ui|{
                    let mut multi=self.title_draft["multicolor"]==true;let mut glowing=self.title_draft["glow_enabled"]!=false;
                    if crate::design::toggle(ui,&mut multi,"Multicolor").changed(){self.title_draft["multicolor"]=json!(multi);changed=true;}
                    if crate::design::toggle(ui,&mut glowing,crate::i18n::t("Con resplandor")).changed(){self.title_draft["glow_enabled"]=json!(glowing);changed=true;}
                });
            });
            ui.add_space(12.);
            d::card().inner_margin(20).show(ui,|ui|{
                ui.label(RichText::new(crate::i18n::t("Base del título")).strong());
                let titles=self.snapshot["titles"].as_array().cloned().unwrap_or_default();
                if let Some(t)=titles.iter().find(|t|t["equipped"]==true){if self.title_draft["target"]!=t["id"]{self.title_draft["target"]=t["id"].clone();changed=true;}d::muted(ui,crate::i18n::t("Se usa automáticamente tu título equipado. Las tarjetas de arriba solo eligen el acabado."));}
                else{d::muted(ui,crate::i18n::t("Equipa cualquier título en Rocket League para utilizarlo como base del texto personalizado."));}

            });
        });
        ui.add_space(10.);
        ui.horizontal_wrapped(|ui|{
            let valid=!s(&self.title_draft,"text").trim().is_empty()&&!s(&self.title_draft,"target").is_empty();
            if ui.add_enabled(!self.busy&&self.game.is_some()&&valid,d::button(crate::i18n::t("Añadir título a cambios"),d::GREEN())).clicked(){self.title_apply(self.title_draft.clone());}
            if ui.add_enabled(!self.busy&&self.profile_state["settings"]["title"].is_object(),d::button(crate::i18n::t("Revertir título"),d::RED())).clicked(){self.title_apply(Value::Null);}
            if self.running{d::muted(ui,crate::i18n::t("Cierra Rocket League para aplicar."));}
        });
        if changed{let _=fs::write(self.root.join("data/title-draft.json"),serde_json::to_vec_pretty(&self.title_draft).unwrap());if !s(&self.title_draft,"text").trim().is_empty()&&!s(&self.title_draft,"target").is_empty(){self.title_apply(self.title_draft.clone());}}
    }
}
