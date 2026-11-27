use super::*;
use crate::design as d;

impl App{
    /// One compact row per sound: play, name, small waveform, «Usar».
    pub(super) fn sound_library(&mut self,ui:&mut egui::Ui){
        let any=self.audio_library.iter().any(|a|s(a,"kind")==self.studio_sound_kind);
        if any{crate::design::toggle(ui,&mut self.only_favorites,crate::i18n::t("Favoritos"));ui.add_space(8.);}
        let items:Vec<_>=self.audio_library.iter().filter(|a|s(a,"kind")==self.studio_sound_kind&&(!self.only_favorites||self.is_favorite(&format!("sound:{}",s(a,"path"))))).cloned().collect();
        if items.is_empty(){d::muted(ui,if any{crate::i18n::t("No tienes favoritos aquí. Toca la estrella de un sonido.")}else{crate::i18n::t("Aún no tienes sonidos. Elige un MP3 o descárgalo.")});return;}
        let current=self.studio_draft["sounds"][&self.studio_sound_kind]["path"].clone();
        egui::ScrollArea::vertical().id_salt("sound-library").show(ui,|ui|{d::card().inner_margin(egui::Margin::symmetric(16,6)).show(ui,|ui|{
            for(n,audio)in items.iter().enumerate(){
                if n>0{let y=ui.cursor().min.y;ui.painter().hline(ui.max_rect().x_range(),y,egui::Stroke::new(1_f32,d::LINE()));}
                ui.horizontal(|ui|{
                    ui.set_min_height(52.);
                    let(b,resp)=ui.allocate_exact_size(Vec2::splat(32.),egui::Sense::click());
                    ui.painter().circle_filled(b.center(),16.,if resp.hovered(){d::TEXT()}else{d::LINE()});
                    let c=b.center();let tri=if resp.hovered(){d::BACKGROUND()}else{d::TEXT()};
                    ui.painter().add(egui::Shape::convex_polygon(vec![c+Vec2::new(-4.,-7.),c+Vec2::new(8.,0.),c+Vec2::new(-4.,7.)],tri,egui::Stroke::NONE));
                    if resp.on_hover_cursor(egui::CursorIcon::PointingHand).on_hover_text(crate::i18n::t("Escuchar")).clicked()&&!self.busy{self.call("sound_preview",json!({"action":"sound_preview","path":audio["path"],"start":audio["start"],"end":audio["end"],"volume":audio["volume"]}));}
                    ui.add_space(4.);
                    let(star,_)=ui.allocate_exact_size(Vec2::splat(24.),egui::Sense::hover());
                    self.favorite_star(ui,star.center(),&format!("sound:{}",s(audio,"path")),true);
                    ui.add_space(2.);
                    ui.vertical(|ui|{ui.add_space(6.);ui.add(egui::Label::new(RichText::new(s(audio,"name"))).truncate());ui.label(RichText::new(format!("{:.1} s",audio["seconds"].as_f64().unwrap_or(0.))).size(12.).color(d::MUTED()));});
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center),|ui|{
                        let chosen=audio["path"]==current;
                        if chosen{ui.label(RichText::new(crate::i18n::t("En uso")).size(13.).color(d::MUTED()));}
                        else if ui.add_enabled(!self.busy,d::button(crate::i18n::t("Usar"),d::TEXT()).min_size(Vec2::new(0.,32.))).clicked(){self.studio_draft["sounds"][&self.studio_sound_kind]=audio.clone();self.save_studio_draft();self.sound_browser=0;self.status=crate::i18n::t("Sonido elegido. Pulsa Aplicar abajo.").into();}
                        ui.add_space(10.);
                        let(w,_)=ui.allocate_exact_size(Vec2::new(140.,24.),egui::Sense::hover());
                        for(i,peak)in audio["peaks"].as_array().into_iter().flatten().enumerate().step_by(3){let x=w.left()+(i as f32+0.5)/96.*w.width();let h=(peak.as_f64().unwrap_or(0.)as f32*22.).max(2.);ui.painter().line_segment([egui::pos2(x,w.center().y-h/2.),egui::pos2(x,w.center().y+h/2.)],egui::Stroke::new(2_f32,d::MUTED().gamma_multiply(0.6)));}
                    });
                });
            }
        });});
    }

    pub(super) fn media_panel(&mut self,ui:&mut egui::Ui,sound:bool){
        let source=if sound{"sounds"}else if self.texture_source==1{"vgm"}else{"alpha"};
        if self.media_source!=source{self.media_source=source.into();self.media_data=read_json(&self.root.join(format!("data/media/{source}.json")));self.media_query.clear();if !self.media_data["items"].is_array()&&!self.busy&&self.design_dir.is_none(){self.call("media_catalog",json!({"action":"media_catalog","source":source,"refresh":true}));}}
        ui.heading(if sound{crate::i18n::t("Descargar sonidos")}else{crate::i18n::t("Cosméticos · Descargar texturas")});
        ui.horizontal_wrapped(|ui|{
            if !sound{ui.selectable_value(&mut self.texture_source,0,"AlphaConsole");ui.selectable_value(&mut self.texture_source,1,"Video Game Mods");}
            if ui.add_enabled(!self.busy,egui::Button::new(crate::i18n::t("Actualizar"))).clicked(){self.call("media_catalog",json!({"action":"media_catalog","source":source,"query":self.media_query,"refresh":true}));}
            ui.hyperlink_to(crate::i18n::t("Abrir fuente"),match source{"alpha"=>"https://alphaconsole.io/browse","vgm"=>"https://videogamemods.com/vgm/rocketleague/mods?page=3",_=>"https://www.myinstants.com/es/search/"});
        });
        ui.horizontal(|ui|{
            let search=ui.add(egui::TextEdit::singleline(&mut self.media_query).hint_text(if sound{crate::i18n::t("Buscar en Myinstants…")}else{crate::i18n::t("Buscar nombre o autor…")}).desired_width((ui.available_width()-115.).max(150.)));
            if sound{if ui.add_enabled(!self.busy,egui::Button::new(crate::i18n::t("Buscar"))).clicked()||(search.lost_focus()&&ui.input(|i|i.key_pressed(egui::Key::Enter))){self.call("media_catalog",json!({"action":"media_catalog","source":source,"query":self.media_query,"refresh":true}));}}
        });
        if !sound{ui.horizontal(|ui|{
            ui.add(egui::TextEdit::singleline(&mut self.media_url).hint_text(crate::i18n::t("Pegar ficha de la fuente para añadir otra textura…")).desired_width((ui.available_width()-155.).max(150.)));
            if ui.add_enabled(!self.busy&&!self.media_url.is_empty(),egui::Button::new(crate::i18n::t("Añadir enlace"))).clicked(){self.call("media_catalog",json!({"action":"media_catalog","source":source,"url":self.media_url,"refresh":true}));}
        });}
        if source=="vgm"{d::muted(ui,crate::i18n::t("Selección inicial de fichas públicas. Añade otras con su enlace; los ZIP se validan antes de importarlos."));}
        let query=self.media_query.to_lowercase();
        let items:Vec<Value>=self.media_data["items"].as_array().into_iter().flatten().filter(|v|sound||(s(v,"category")==self.texture_category&&format!("{} {}",s(v,"name"),s(v,"author")).to_lowercase().contains(&query))).cloned().collect();
        d::muted(ui,crate::i18n::tr(&format!("{} resultados · Descargar guarda en tu biblioteca; Elegir lo añade a los cambios sin aplicar.",items.len())));
        if items.is_empty(){ui.add_space(14.);d::muted(ui,if self.busy{crate::i18n::t("Cargando contenido…")}else{crate::i18n::t("Pulsa Actualizar o prueba otra búsqueda. También puedes importar tus archivos locales.")});return;}
        let cols=((ui.available_width()+12.)/260.).floor().max(1.)as usize;
        egui::ScrollArea::vertical().id_salt(("media-grid",sound)).show(ui,|ui|{for chunk in items.chunks(cols){ui.columns(cols,|columns|{for(ui,item)in columns.iter_mut().zip(chunk){d::card().inner_margin(12).show(ui,|ui|{
            if !sound{let(rect,_)=ui.allocate_exact_size(Vec2::new(ui.available_width(),138.),egui::Sense::hover());ui.painter().rect_filled(rect,6,d::BACKGROUND());if let Some(path)=item["preview"].as_str(){egui::Image::from_uri(format!("file://{path}")).fit_to_exact_size(rect.size()).corner_radius(6).paint_at(ui,rect);}else{crate::icons::paint(ui,egui::Rect::from_center_size(rect.center(),Vec2::splat(32.)),crate::i18n::t("Texturas"),d::MUTED());}}
            ui.add(egui::Label::new(RichText::new(s(item,"name")).strong()).truncate()).on_hover_text(s(item,"name"));
            d::muted(ui,if s(item,"author").is_empty(){s(item,"source")}else{s(item,"author")});
            if sound&&ui.add_enabled(!self.busy,egui::Button::new(crate::i18n::t("▶ Preescucha"))).clicked(){self.call("media_preview",json!({"action":"media_preview","key":item["key"],"kind":self.studio_sound_kind}));}
            let ready=if sound{self.audio_library.iter().any(|a|s(a,"kind")==self.studio_sound_kind&&a["catalog_key"]==item["key"])}else{item["local"]==true};
            if ready{
                if ui.add_enabled(!self.busy,d::button(crate::i18n::t("Ver en mi biblioteca"),d::GREEN())).clicked(){if sound{self.sound_browser=1;}else{self.texture_shop=false;self.pack_query.clear();}}
            }else if !s(item,"download").is_empty(){
                if ui.add_enabled(!self.busy,egui::Button::new(crate::i18n::t("↓ Descargar"))).clicked(){self.call("media_download",json!({"action":"media_download","source":source,"key":item["key"],"kind":self.studio_sound_kind}));}
            }
            ui.hyperlink_to(crate::i18n::t("Ver ficha"),s(item,"page"));
        });}});ui.add_space(8.);}});
    }
}
