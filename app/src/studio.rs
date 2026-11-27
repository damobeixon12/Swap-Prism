use super::*;
use crate::design as d;

fn hex(c: Color32)->String{format!("#{:02X}{:02X}{:02X}",c.r(),c.g(),c.b())}
fn color(s:&str)->Color32{u32::from_str_radix(s.trim_start_matches('#'),16).map(|n|Color32::from_rgb((n>>16)as u8,(n>>8)as u8,n as u8)).unwrap_or(d::CYAN())}
fn rich_colors(columns:usize)->Vec<Value>{
    (0..7).flat_map(|row|(0..columns).map(move|col|{
        let v=[0.94,0.81,0.65,0.49,0.33,0.19,0.08][row];
        let rgb=if col==0{[v,v,v]}else{egui::ecolor::Hsva::new((col-1)as f32/(columns-1)as f32,if row==0{0.35}else{0.94},v,1.).to_srgb().map(|x|x as f32/255.)};
        json!(hex(Color32::from_rgb((rgb[0]*255.)as u8,(rgb[1]*255.)as u8,(rgb[2]*255.)as u8)))
    })).collect()
}
impl App{
    pub fn studio_dirty(&self)->bool{
        fn effective(value:&Value)->Value{
            let mut out=json!({});
            if value["palette"]["enabled"]==true{out["palette"]=value["palette"].clone();}
            // Own key: "reset" below is the reset sound.
            if value["reset_enabled"]==true{out["reset_color"]=value["reset_color"].clone();}
            if value["season_enabled"]==true{out["season"]=value["season"].clone();}
            for key in ["reset","post"]{if value["sounds"][key].is_object(){out[key]=value["sounds"][key].clone();}}
            out
        }
        effective(&self.studio_draft)!=effective(&self.studio_applied)
    }
    pub fn save_studio_draft(&self){if self.design_dir.is_none(){let _=fs::write(self.root.join("data/studio-draft.json"),serde_json::to_vec_pretty(&self.studio_draft).unwrap());}}
    pub fn studio_editor(&mut self,ui:&mut egui::Ui){
        if self.studio_tab==1{
            ui.horizontal(|ui|{ui.selectable_value(&mut self.sound_browser,1,"Mi biblioteca");ui.selectable_value(&mut self.sound_browser,0,"Actual");ui.selectable_value(&mut self.sound_browser,2,"Descargar");});
            if self.sound_browser==1{self.sound_library(ui);return;}
            if self.sound_browser==2{self.media_panel(ui,true);return;}
        }
        let before=self.studio_draft.clone();
        if !self.studio_draft.is_object(){self.studio_draft=json!({});}
        if !self.studio_draft["palette"].is_object(){self.studio_draft["palette"]=json!({"enabled":false});}
        if !self.studio_draft["sounds"].is_object(){self.studio_draft["sounds"]=json!({});}
        for(key,columns)in [("blue",10),("orange",10),("accent",15)]{if !self.studio_draft["palette"][key].is_array(){self.studio_draft["palette"][key]=json!(rich_colors(columns));}}
        egui::ScrollArea::vertical().id_salt("studio-scroll").max_height((ui.available_height()-70.).max(100.)).show(ui,|ui|{
            match self.studio_tab {
                0=>{
                    
                    let mut enabled=self.studio_draft["palette"]["enabled"]==true;ui.checkbox(&mut enabled,"Usar más colores en el garaje");self.studio_draft["palette"]["enabled"]=json!(enabled);
                    ui.horizontal_wrapped(|ui|{for(key,label)in [("blue","Equipo azul · primario"),("orange","Equipo naranja · primario"),("accent","Acento · ambos equipos")]{if ui.selectable_label(self.studio_palette==key,label).clicked(){self.studio_palette=key.into();self.studio_slot=0;}}});
                    let key=self.studio_palette.clone();let columns=if key=="accent"{15}else{10};
                    if self.studio_draft["palette"][&key].as_array().map(|v|v.len())!=Some(columns*7){self.studio_draft["palette"][&key]=json!(rich_colors(columns));}
                    d::card().show(ui,|ui|{
                        d::eyebrow(ui,"ELIGE UN COLOR PARA EDITARLO",d::CYAN());
                        let colors=self.studio_draft["palette"][&key].as_array().cloned().unwrap_or_default();
                        let size=((ui.available_width()-(columns-1)as f32*5.)/columns as f32).min(27.).max(15.);
                        ui.spacing_mut().item_spacing=Vec2::splat(5.);
                        ui.spacing_mut().interact_size.y=size;
                        for row in 0..7{ui.horizontal(|ui|{for col in 0..columns{let i=row*columns+col;let c=color(colors[i].as_str().unwrap_or("#FFFFFF"));let(r,response)=ui.allocate_exact_size(Vec2::splat(size),egui::Sense::click());ui.painter().rect_filled(r,5,c);if self.studio_slot==i{ui.painter().rect_stroke(r.expand(2.),6,egui::Stroke::new(2_f32,d::TEXT()),egui::StrokeKind::Outside);}if response.clicked(){self.studio_slot=i;}}});}
                        ui.spacing_mut().interact_size.y=38.;
                        ui.add_space(14.);let mut c=color(colors[self.studio_slot].as_str().unwrap_or("#FFFFFF"));let mut channels=[c.r(),c.g(),c.b()];
                        ui.horizontal(|ui|{ui.label(format!("Casilla {}",self.studio_slot+1));if ui.color_edit_button_srgb(&mut channels).changed(){c=Color32::from_rgb(channels[0],channels[1],channels[2]);self.studio_draft["palette"][&key][self.studio_slot]=json!(hex(c));}ui.label(hex(c));});
                        if ui.button("Restablecer este panel a Rich").clicked(){self.studio_draft["palette"][&key]=json!(rich_colors(columns));}
                    });
                    d::muted(ui,"En el juego, desactiva «Forzar colores predeterminados» para verlos.");
                },
                1=>{
                    let specs:Value=serde_json::from_slice(include_bytes!("../assets/audio_manifest.json")).unwrap();
                    let kind=if self.studio_sound_kind=="post"{"post"}else{"reset"};
                    let label=if kind=="post"{"Poste y travesaño"}else{"Flip reset"};
                    let limit=specs[kind]["max_seconds"].as_f64().unwrap_or(0.0)as f32;
                    let allowed=specs[kind]["disabled_reason"].is_null();
                    if let Some(reason)=specs[kind]["disabled_reason"].as_str(){d::muted(ui,reason);ui.add_space(8.);}
                    let selected=self.studio_draft["sounds"][kind].clone();
                    let total=selected["seconds"].as_f64().unwrap_or(0.0)as f32;
                    let mut pick=false;
                    if selected.is_object()&&total>0.0{
                        let mut start=selected["start"].as_f64().unwrap_or(0.0)as f32;
                        let mut end=selected["end"].as_f64().unwrap_or(total.min(limit)as f64)as f32;
                        let mut volume=selected["volume"].as_f64().unwrap_or(1.0)as f32;
                        let (mut play,mut volume_changed)=(false,false);
                        d::card().show(ui,|ui|{
                            // Player: big play button, name and length of the clip.
                            ui.horizontal(|ui|{
                                let(b,resp)=ui.allocate_exact_size(Vec2::splat(56.),egui::Sense::click());
                                let playing=self.audio_preview.as_ref().is_some_and(|p|p.0==kind&&p.1.elapsed().as_secs_f32()<p.2);
                                let fill=if self.busy{d::LINE()}else if resp.hovered(){d::GREEN().gamma_multiply(0.85)}else{d::GREEN()};
                                ui.painter().circle_filled(b.center(),28.,fill);
                                let c=b.center();
                                if playing{for dx in [-6.,6.]{ui.painter().rect_filled(egui::Rect::from_center_size(c+Vec2::new(dx,0.),Vec2::new(6.,20.)),2,d::ON_ACCENT());}}
                                else{ui.painter().add(egui::Shape::convex_polygon(vec![c+Vec2::new(-6.,-11.),c+Vec2::new(12.,0.),c+Vec2::new(-6.,11.)],d::ON_ACCENT(),egui::Stroke::NONE));}
                                if resp.on_hover_cursor(egui::CursorIcon::PointingHand).on_hover_text("Escuchar").clicked()&&!self.busy{play=true;}
                                ui.add_space(12.);
                                ui.vertical(|ui|{
                                    let mut name=s(&selected,"name").to_owned();
                                    if ui.add(egui::TextEdit::singleline(&mut name).frame(false).font(d::bold(18.)).hint_text(label).char_limit(64).desired_width(f32::INFINITY)).changed(){self.studio_draft["sounds"][kind]["name"]=json!(name);}
                                    d::muted(ui,format!("{:.1} s de {:.1} s",end-start,total));
                                });
                            });
                            ui.add_space(16.);
                            // Waveform: drag the edges to trim, click to move the selection.
                            let(r,response)=ui.allocate_exact_size(Vec2::new(ui.available_width(),78.),egui::Sense::click());ui.painter().rect_filled(r,10,d::BACKGROUND());
                            let x=|t:f32|r.left()+12.+t/total*(r.width()-24.);
                            ui.painter().rect_filled(egui::Rect::from_min_max(egui::pos2(x(start),r.top()+6.),egui::pos2(x(end),r.bottom()-6.)),6,d::GREEN().gamma_multiply(0.12));
                            for i in 0..96{let peak=selected["peaks"][i].as_f64().unwrap_or(0.0)as f32;let t=(i as f32+0.5)/96.*total;let h=(peak*54.).max(2.);let xx=x(t);ui.painter().line_segment([egui::pos2(xx,r.center().y-h/2.),egui::pos2(xx,r.center().y+h/2.)],egui::Stroke::new(2.5_f32,if t>=start&&t<=end{d::GREEN()}else{d::MUTED().gamma_multiply(0.35)}));}
                            for(which,t)in [(0,start),(1,end)]{
                                let xx=x(t);let handle=egui::Rect::from_center_size(egui::pos2(xx,r.center().y),Vec2::new(14.,r.height()-8.));
                                ui.painter().line_segment([egui::pos2(xx,r.top()+6.),egui::pos2(xx,r.bottom()-6.)],egui::Stroke::new(2_f32,d::TEXT()));
                                ui.painter().circle_filled(egui::pos2(xx,r.top()+8.),4.,d::TEXT());
                                let drag=ui.interact(handle,ui.id().with((kind,which)),egui::Sense::drag()).on_hover_cursor(egui::CursorIcon::ResizeHorizontal);
                                if drag.dragged(){if let Some(pos)=drag.interact_pointer_pos(){let t=((pos.x-r.left()-12.)/(r.width()-24.)*total).clamp(0.,total);if which==0{start=t.min(end-0.01).max((end-limit).max(0.));}else{end=t.max(start+0.01).min((start+limit).min(total));}}}
                            }
                            if response.clicked(){if let Some(pos)=response.interact_pointer_pos(){let length=(end-start).max(0.01);start=((pos.x-r.left()-12.)/(r.width()-24.)*total).clamp(0.,(total-length).max(0.));end=(start+length).min(total);}}
                            if let Some((playing,began,length))=&self.audio_preview{if playing==kind{ui.ctx().request_repaint_after(Duration::from_millis(16));let elapsed=began.elapsed().as_secs_f32().min(*length);let xx=x(start+elapsed);ui.painter().line_segment([egui::pos2(xx,r.top()),egui::pos2(xx,r.bottom())],egui::Stroke::new(2_f32,d::TEXT()));}}
                            ui.add_space(18.);
                            ui.horizontal(|ui|{
                                ui.label(RichText::new("Volumen").color(d::MUTED()));
                                ui.spacing_mut().slider_width=(ui.available_width()-70.).max(80.);
                                if ui.add(egui::Slider::new(&mut volume,0.0..=if kind=="post"{2.0}else{1.0}).show_value(false)).changed(){volume_changed=true;}
                                ui.label(RichText::new(format!("{} %",(volume*100.).round()as u32)).strong());
                            });
                        });
                        self.studio_draft["sounds"][kind]["start"]=json!(start);self.studio_draft["sounds"][kind]["end"]=json!(end);
                        if volume_changed{self.studio_draft["sounds"][kind]["volume"]=json!(volume);}
                        if play{self.audio_preview=Some((kind.to_owned(),Instant::now(),end-start));self.call("sound_preview",json!({"action":"sound_preview","path":selected["path"],"volume":volume,"start":start,"end":end}));}
                        ui.add_space(14.);
                        ui.horizontal(|ui|{
                            if ui.add_enabled(!self.busy&&allowed,d::primary("Cambiar sonido")).clicked(){pick=true;}
                            if ui.add_enabled(!self.busy,d::button("Volver al original",d::MUTED())).clicked(){self.studio_draft["sounds"][kind]=Value::Null;}
                        });
                    }else{
                        d::card().show(ui,|ui|{ui.vertical_centered(|ui|{
                            ui.add_space(22.);
                            let(r,_)=ui.allocate_exact_size(Vec2::splat(56.),egui::Sense::hover());
                            ui.painter().circle_filled(r.center(),28.,d::GREEN().gamma_multiply(0.14));
                            crate::icons::paint(ui,egui::Rect::from_center_size(r.center(),Vec2::splat(26.)),"sonido del reset",d::GREEN());
                            ui.add_space(12.);
                            ui.label(RichText::new(label).font(d::bold(19.)));
                            d::muted(ui,format!("Sonido original del juego. Usa un MP3 tuyo (hasta {:.1} s).",limit));
                            ui.add_space(14.);
                            if ui.add_enabled(!self.busy&&allowed,d::primary("Elegir MP3")).clicked(){pick=true;}
                            ui.add_space(22.);
                        });});
                    }
                    if pick{if let Some(path)=rfd::FileDialog::new().add_filter("MP3",&["mp3"]).pick_file(){self.call("sound_import",json!({"action":"sound_import","path":path,"kind":kind}));}}
                },
                2=>{
                    let c=if s(&self.studio_draft,"reset_color").is_empty(){d::CYAN()}else{color(s(&self.studio_draft,"reset_color"))};
                    let mut rgb=[c.r(),c.g(),c.b()];
                    let mut enabled=self.studio_draft["reset_enabled"]==true;
                    const PRESETS:[[u8;3];10]=[[226,75,74],[239,159,39],[250,220,80],[151,196,89],[93,202,165],[55,138,221],[127,119,221],[212,83,126],[240,240,240],[30,30,34]];
                    ui.columns(2,|cols|{
                        let ui=&mut cols[0];
                        ui.checkbox(&mut enabled,"Usar mi color");
                        ui.add_space(14.);
                        for row in PRESETS.chunks(5){
                            ui.horizontal(|ui|{ui.spacing_mut().item_spacing.x=10.;for p in row{
                                let(r,resp)=ui.allocate_exact_size(Vec2::splat(40.),egui::Sense::click());
                                ui.painter().rect_filled(r,10,Color32::from_rgb(p[0],p[1],p[2]));
                                if *p==rgb{ui.painter().rect_stroke(r.expand(3.),12,egui::Stroke::new(2_f32,d::TEXT()),egui::StrokeKind::Outside);}
                                if resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked(){rgb=*p;enabled=true;}
                            }});
                            ui.add_space(10.);
                        }
                        ui.horizontal(|ui|{if ui.color_edit_button_srgb(&mut rgb).changed(){enabled=true;}d::muted(ui,"Otro color");});
                        let ui=&mut cols[1];
                        let(r,_)=ui.allocate_exact_size(Vec2::new(ui.available_width(),230.),egui::Sense::hover());
                        ui.painter().rect_filled(r,14,d::BACKGROUND());
                        let shown=if enabled{Color32::from_rgb(rgb[0],rgb[1],rgb[2])}else{d::CYAN()};
                        let center=r.center()-Vec2::new(0.,14.);
                        for i in (1..=5).rev(){ui.painter().circle_stroke(center,24.+i as f32*9.,egui::Stroke::new(3_f32,shown.gamma_multiply(1./i as f32)));}
                        ui.painter().text(egui::pos2(r.center().x,r.bottom()-20.),egui::Align2::CENTER_CENTER,if enabled{"Así se verá en el juego"}else{"Color original"},egui::FontId::proportional(13.),d::MUTED());
                    });
                    self.studio_draft["reset_enabled"]=json!(enabled);
                    self.studio_draft["reset_color"]=json!(hex(Color32::from_rgb(rgb[0],rgb[1],rgb[2])));
                    ui.add_space(12.);
                    d::muted(ui,"En Rocket League deja activados los efectos del flip reset.");
                },
                _=>{
                    
                    d::card().show(ui,|ui|{
                        let mut enabled=self.studio_draft["season_enabled"]==true;ui.checkbox(&mut enabled,"Usar diseño Swap Prism");self.studio_draft["season_enabled"]=json!(enabled);
                        let mut season=self.studio_draft["season"].as_u64().unwrap_or(24)as u32;
                        ui.horizontal(|ui|{ui.label("Temporada del diseño");ui.add(egui::DragValue::new(&mut season).range(1..=999));});self.studio_draft["season"]=json!(season);
                        let(r,_)=ui.allocate_exact_size(Vec2::new(ui.available_width(),190.),egui::Sense::hover());ui.painter().rect_filled(r,12,d::BACKGROUND());
                        let logo=egui::Rect::from_center_size(r.center()-Vec2::new(0.,30.),Vec2::splat(72.));egui::Image::new(egui::include_image!("../assets/logo.png")).paint_at(ui,logo);
                        ui.painter().text(r.center()+Vec2::new(0.,24.),egui::Align2::CENTER_CENTER,"SWAP PRISM × ROCKET LEAGUE",egui::FontId::proportional(18.),d::CYAN());
                        ui.painter().text(r.center()+Vec2::new(0.,62.),egui::Align2::CENTER_CENTER,format!("SEASON {season}"),egui::FontId::proportional(30.),d::TEXT());
                    });
                }
            }
        });
        ui.add_space(10.);ui.horizontal_wrapped(|ui|{
            
            if ui.add_enabled(!self.busy,d::button("Quitar personalización",d::RED())).clicked(){self.studio_draft=json!({});}
            if self.running{d::muted(ui,"Puedes elegir ahora y aplicar al cerrar Rocket League.");}
        });
        if self.studio_draft!=before{self.save_studio_draft();}
    }
}
