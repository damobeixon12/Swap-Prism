use super::*;
use crate::design as d;

impl App{
    pub(super) fn sound_library(&mut self,ui:&mut egui::Ui){
        ui.heading("Mi biblioteca de sonidos");d::muted(ui,"Escucha el fragmento o elígelo. Puedes ajustar su recorte en Actual.");
        let items:Vec<_>=self.audio_library.iter().filter(|a|s(a,"kind")==self.studio_sound_kind).cloned().collect();
        egui::ScrollArea::vertical().id_salt("sound-library").show(ui,|ui|{
            if items.is_empty(){d::muted(ui,"Todavía no hay sonidos para este evento. Importa un MP3 o entra en Descargar.");}
            for audio in items{d::card().inner_margin(14).show(ui,|ui|{
                ui.horizontal(|ui|{ui.label(RichText::new(s(&audio,"name")).strong());d::muted(ui,format!("{:.2} s",audio["seconds"].as_f64().unwrap_or(0.)));});
                let(rect,_)=ui.allocate_exact_size(Vec2::new(ui.available_width(),42.),egui::Sense::hover());
                for(i,peak)in audio["peaks"].as_array().into_iter().flatten().enumerate(){let x=rect.left()+(i as f32+0.5)/96.*rect.width();let h=(peak.as_f64().unwrap_or(0.)as f32*34.).max(2.);ui.painter().line_segment([egui::pos2(x,rect.center().y-h/2.),egui::pos2(x,rect.center().y+h/2.)],egui::Stroke::new(2_f32,d::MUTED()));}
                ui.horizontal_wrapped(|ui|{
                    if ui.add_enabled(!self.busy,egui::Button::new("▶ Escuchar")).clicked(){self.call("sound_preview",json!({"action":"sound_preview","path":audio["path"],"start":audio["start"],"end":audio["end"],"volume":audio["volume"]}));}
                    if ui.add_enabled(!self.busy,d::button("Elegir",d::GREEN())).clicked(){self.studio_draft["sounds"][&self.studio_sound_kind]=audio.clone();self.save_studio_draft();self.sound_browser=0;self.status="Sonido añadido a Aplicar cambios. Puedes ajustar el recorte en Actual.".into();}
                });
            });ui.add_space(8.);}
        });
    }

    pub(super) fn media_panel(&mut self,ui:&mut egui::Ui,sound:bool){
        let source=if sound{"sounds"}else if self.texture_source==1{"vgm"}else{"alpha"};
        if self.media_source!=source{self.media_source=source.into();self.media_data=read_json(&self.root.join(format!("data/media/{source}.json")));self.media_query.clear();if !self.media_data["items"].is_array()&&!self.busy&&self.design_dir.is_none(){self.call("media_catalog",json!({"action":"media_catalog","source":source,"refresh":true}));}}
        ui.heading(if sound{"Descargar sonidos"}else{"Cosméticos · Descargar texturas"});
        ui.horizontal_wrapped(|ui|{
            if !sound{ui.selectable_value(&mut self.texture_source,0,"AlphaConsole");ui.selectable_value(&mut self.texture_source,1,"Video Game Mods");}
            if ui.add_enabled(!self.busy,egui::Button::new("Actualizar")).clicked(){self.call("media_catalog",json!({"action":"media_catalog","source":source,"query":self.media_query,"refresh":true}));}
            ui.hyperlink_to("Abrir fuente",match source{"alpha"=>"https://alphaconsole.io/browse","vgm"=>"https://videogamemods.com/vgm/rocketleague/mods?page=3",_=>"https://www.myinstants.com/es/search/"});
        });
        ui.horizontal(|ui|{
            let search=ui.add(egui::TextEdit::singleline(&mut self.media_query).hint_text(if sound{"Buscar en Myinstants…"}else{"Buscar nombre o autor…"}).desired_width((ui.available_width()-115.).max(150.)));
            if sound{if ui.add_enabled(!self.busy,egui::Button::new("Buscar")).clicked()||(search.lost_focus()&&ui.input(|i|i.key_pressed(egui::Key::Enter))){self.call("media_catalog",json!({"action":"media_catalog","source":source,"query":self.media_query,"refresh":true}));}}
        });
        if !sound{ui.horizontal(|ui|{
            ui.add(egui::TextEdit::singleline(&mut self.media_url).hint_text("Pegar ficha de la fuente para añadir otra textura…").desired_width((ui.available_width()-155.).max(150.)));
            if ui.add_enabled(!self.busy&&!self.media_url.is_empty(),egui::Button::new("Añadir enlace")).clicked(){self.call("media_catalog",json!({"action":"media_catalog","source":source,"url":self.media_url,"refresh":true}));}
        });}
        if source=="vgm"{d::muted(ui,"Selección inicial de fichas públicas. Añade otras con su enlace; los ZIP se validan antes de importarlos.");}
        let query=self.media_query.to_lowercase();
        let items:Vec<Value>=self.media_data["items"].as_array().into_iter().flatten().filter(|v|sound||(s(v,"category")==self.texture_category&&format!("{} {}",s(v,"name"),s(v,"author")).to_lowercase().contains(&query))).cloned().collect();
        d::muted(ui,format!("{} resultados · Descargar guarda en tu biblioteca; Elegir lo añade a los cambios sin aplicar.",items.len()));
        if items.is_empty(){ui.add_space(14.);d::muted(ui,if self.busy{"Cargando contenido…"}else{"Pulsa Actualizar o prueba otra búsqueda. También puedes importar tus archivos locales."});return;}
        let cols=((ui.available_width()+12.)/260.).floor().max(1.)as usize;
        egui::ScrollArea::vertical().id_salt(("media-grid",sound)).show(ui,|ui|{for chunk in items.chunks(cols){ui.columns(cols,|columns|{for(ui,item)in columns.iter_mut().zip(chunk){d::card().inner_margin(12).show(ui,|ui|{
            if !sound{let(rect,_)=ui.allocate_exact_size(Vec2::new(ui.available_width(),138.),egui::Sense::hover());ui.painter().rect_filled(rect,6,d::BACKGROUND());if let Some(path)=item["preview"].as_str(){egui::Image::from_uri(format!("file://{path}")).fit_to_exact_size(rect.size()).corner_radius(6).paint_at(ui,rect);}else{crate::icons::paint(ui,egui::Rect::from_center_size(rect.center(),Vec2::splat(32.)),"Texturas",d::MUTED());}}
            ui.add(egui::Label::new(RichText::new(s(item,"name")).strong()).truncate()).on_hover_text(s(item,"name"));
            d::muted(ui,if s(item,"author").is_empty(){s(item,"source")}else{s(item,"author")});
            if sound&&ui.add_enabled(!self.busy,egui::Button::new("▶ Preescucha")).clicked(){self.call("media_preview",json!({"action":"media_preview","key":item["key"],"kind":self.studio_sound_kind}));}
            let ready=if sound{self.audio_library.iter().any(|a|s(a,"kind")==self.studio_sound_kind&&a["catalog_key"]==item["key"])}else{item["local"]==true};
            if ready{
                if ui.add_enabled(!self.busy,d::button("Ver en mi biblioteca",d::GREEN())).clicked(){if sound{self.sound_browser=1;}else{self.texture_shop=false;self.pack_query.clear();}}
            }else if !s(item,"download").is_empty(){
                if ui.add_enabled(!self.busy,egui::Button::new("↓ Descargar")).clicked(){self.call("media_download",json!({"action":"media_download","source":source,"key":item["key"],"kind":self.studio_sound_kind}));}
            }
            ui.hyperlink_to("Ver ficha",s(item,"page"));
        });}});ui.add_space(8.);}});
    }
}
