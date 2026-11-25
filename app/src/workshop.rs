use super::*;
use crate::design as d;

impl App {
    pub(super) fn workshop_editor(&mut self,ui:&mut egui::Ui){
        let installed=self.state["Labs_Utopia_P.upk"].clone();
        let active=installed["status"]=="installed" && s(&installed,"observed_status")!="original";
        ui.horizontal_wrapped(|ui|{
            ui.vertical(|ui|{ui.label(RichText::new("Workshop Maps").strong().size(17.));d::muted(ui,if active{format!("Activo: {}",s(&installed["donor_item"],"Product"))}else{"Destino fijo: Utopia Retro · Balón rueda".into()});});
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center),|ui|{
                if ui.add_enabled(!self.busy,egui::Button::new("Actualizar")).clicked(){self.call("workshop_catalog",json!({"action":"workshop_catalog","refresh":true}));}
                if ui.add_enabled(!self.busy,egui::Button::new("Importar…")).clicked(){if let Some(path)=rfd::FileDialog::new().add_filter("Mapas",&["zip","udk","upk"]).pick_file(){self.call("workshop_import",json!({"action":"workshop_import","path":path}));}}
                if ui.add_enabled(active&&!self.busy,egui::Button::new(RichText::new("Restaurar original").color(d::RED()))).clicked(){self.queue_plan("workshop".into(),json!({"kind":"workshop_restore","category":"Workshop","target":"Labs_Utopia_P.upk","label":"Restaurar Utopia Retro"}));self.save_queue();}
            });
        });
        ui.add_space(8.);
        ui.horizontal(|ui|{
            ui.add(egui::TextEdit::singleline(&mut self.workshop_query).hint_text("Buscar mapa…").desired_width((ui.available_width()-330.).max(180.)));
            egui::ComboBox::from_id_salt("workshop-sort").selected_text(if self.workshop_sort==0{"Más descargados"}else{"Nombre"}).show_ui(ui,|ui|{ui.selectable_value(&mut self.workshop_sort,0,"Más descargados");ui.selectable_value(&mut self.workshop_sort,1,"Nombre");});
            ui.checkbox(&mut self.workshop_downloaded,"Descargados");
        });
        d::muted(ui,"Solo reemplaza Utopia Retro. Después de aplicar, ábrelo en Entrenamiento libre → Balón rueda. Los demás estadios se conservan.");
        let query=self.workshop_query.to_lowercase();
        let mut maps:Vec<Value>=self.workshop_data["maps"].as_array().into_iter().flatten().filter(|m|(!self.workshop_downloaded||m["local"].is_object())&&format!("{} {}",s(m,"name"),s(m,"description")).to_lowercase().contains(&query)).cloned().collect();
        maps.sort_by(|a,b|if self.workshop_sort==0{b["downloads"].as_u64().unwrap_or(0).cmp(&a["downloads"].as_u64().unwrap_or(0)).then(s(a,"name").cmp(s(b,"name")))}else{s(a,"name").cmp(s(b,"name"))});
        d::muted(ui,format!("{} mapas · Fuente: Lethamyr.com / importados",maps.len()));
        if maps.is_empty(){d::card().show(ui,|ui|{ui.label(if self.busy{"Cargando catálogo e imágenes…"}else{"No hay mapas para este filtro. Actualiza el catálogo o importa un ZIP / UDK / UPK."});});return;}
        let columns=((ui.available_width()+12.)/235.).floor().max(1.) as usize;
        egui::ScrollArea::vertical().id_salt("workshop-grid").show(ui,|ui|{
            for chunk in maps.chunks(columns){ui.columns(columns,|uis|{for(ui,item)in uis.iter_mut().zip(chunk){
                let key=s(item,"key");let downloaded=item["local"].is_object();
                let is_active=active && installed["installed_sha256"]==item["local"]["sha256"];
                let is_pending=self.plans.get("workshop").map(|p|p["map_key"]==item["key"]).unwrap_or(false);
                d::card().inner_margin(8).show(ui,|ui|{
                    ui.set_min_width(ui.available_width());
                    let(rect,_)=ui.allocate_exact_size(Vec2::new(ui.available_width(),115.),egui::Sense::hover());
                    ui.painter().rect_filled(rect,5,d::FIELD());
                    if let Some(path)=item["preview"].as_str(){egui::Image::from_uri(format!("file://{path}")).fit_to_exact_size(rect.size()).corner_radius(5).paint_at(ui,rect);}else{crate::icons::paint(ui,egui::Rect::from_center_size(rect.center(),Vec2::splat(28.)),"Workshop",d::MUTED());}
                    if is_active||is_pending{ui.painter().rect_filled(egui::Rect::from_min_size(rect.min+Vec2::splat(6.),Vec2::new(78.,22.)),4,d::GREEN());ui.painter().text(rect.min+Vec2::new(45.,17.),egui::Align2::CENTER_CENTER,if is_pending{"PENDIENTE"}else{"ACTIVO"},egui::FontId::proportional(10.),d::BACKGROUND());}
                    ui.add(egui::Label::new(RichText::new(s(item,"name")).strong()).truncate()).on_hover_text(s(item,"description"));
                    d::muted(ui,s(item,"source"));
                    if downloaded{ui.horizontal(|ui|{
                        if ui.add_enabled(!self.busy&&!is_active&&!is_pending,egui::Button::new(RichText::new("Quitar").color(d::RED()))).clicked(){self.call("workshop_remove",json!({"action":"workshop_remove","key":key}));}
                        if ui.add_enabled(!self.busy,egui::Button::new(RichText::new(if is_pending{"En cola"}else if is_active{"Activo"}else{"Elegir"}).color(d::GREEN()))).clicked()&&!is_active&&!is_pending{self.queue_plan("workshop".into(),json!({"kind":"workshop","category":"Workshop","target":"Labs_Utopia_P.upk","map_key":key,"label":format!("Mapa → {}",s(item,"name"))}));self.save_queue();self.status="Mapa añadido a Aplicar cambios. Sustituye únicamente Utopia Retro.".into();}
                    });}else if ui.add_enabled(!self.busy,egui::Button::new("↓ Descargar").min_size(Vec2::new(ui.available_width(),30.))).clicked(){self.call("workshop_download",json!({"action":"workshop_download","key":key}));}
                });
            }});ui.add_space(8.);}
        });
    }
}
