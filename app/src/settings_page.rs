//! «Mis ajustes» (camera, controller, video from the local save), «Jugar» (launcher) and «Mis rangos».
use super::*;
use crate::design as d;

/// (key, label, min, max, step, value shown when the game left it at its default)
const CAMERA:[(&str,&str,f64,f64,f64,f64);7]=[
    ("camera.fov","Campo de visión",60.,110.,1.,90.),("camera.distance","Distancia",100.,400.,10.,270.),("camera.height","Altura",40.,200.,10.,100.),
    ("camera.pitch","Ángulo",-15.,0.,1.,-3.),("camera.stiffness","Rigidez",0.,1.,0.05,0.45),("camera.swivel","Velocidad de giro",1.,10.,0.1,2.5),
    ("camera.transition","Velocidad de transición",1.,2.,0.1,1.2),
];
const CONTROLS:[(&str,&str,f64,f64,f64,f64);5]=[
    ("controls.deadzone","Zona muerta del mando",0.,1.,0.01,0.2),("controls.dodge_deadzone","Zona muerta de esquive",0.,1.,0.01,0.5),
    ("controls.steering","Sensibilidad de dirección",0.1,10.,0.01,1.),("controls.aerial","Sensibilidad aérea",0.1,10.,0.01,1.),
    ("controls.vibration","Intensidad de vibración",0.,1.,0.05,0.),
];
const FPS:[i64;9]=[30,60,120,144,165,200,240,250,360];
const RANK_MODES:[(i64,&str);8]=[(10,"1v1 · Duel"),(11,"2v2 · Doubles"),(13,"3v3 · Standard"),(34,"Torneos"),(27,"Hoops"),(28,"Rumble"),(29,"Dropshot"),(30,"Snow Day")];

fn ini_bool(v:&Value)->bool{v.as_str().map(|s|s.eq_ignore_ascii_case("true")).unwrap_or(v.as_bool().unwrap_or(false))}
fn ini_int(v:&Value)->i64{v.as_str().and_then(|s|s.parse().ok()).or(v.as_i64()).unwrap_or(0)}

impl App{
    pub(super) fn ensure_game_settings(&mut self){
        if self.game_settings.is_null()&&!self.settings_loading&&!self.busy{self.settings_loading=true;self.call("game_settings",json!({"action":"game_settings"}));}
    }

    fn setting_changes(&self)->serde_json::Map<String,Value>{
        let base=&self.game_settings["values"];let mut out=serde_json::Map::new();
        if let Some(draft)=self.settings_draft.as_object(){for(k,v)in draft{if base[k]!=*v{out.insert(k.clone(),v.clone());}}}
        out
    }

    fn slider_row(&mut self,ui:&mut egui::Ui,(key,label,min,max,step,default):(&str,&str,f64,f64,f64,f64)){
        let current=self.settings_draft[key].as_f64();let mut value=current.unwrap_or(default);
        ui.horizontal(|ui|{
            ui.allocate_ui_with_layout(Vec2::new(220.,28.),egui::Layout::left_to_right(egui::Align::Center),|ui|{
                ui.set_min_width(220.);ui.label(RichText::new(label).size(15.));
                if current.is_none(){ui.label(RichText::new("predet.").size(12.).color(d::MUTED()));}
            });
            ui.spacing_mut().slider_width=(ui.available_width()-90.).clamp(160.,460.);
            let decimals=if step>=1.{0}else if step>=0.1{1}else{2};
            if ui.add(egui::Slider::new(&mut value,min..=max).step_by(step).fixed_decimals(decimals)).changed(){self.settings_draft[key]=json!(value);}
        });
        ui.add_space(4.);
    }

    pub(super) fn my_settings(&mut self,ui:&mut egui::Ui){
        self.ensure_game_settings();
        if self.game_settings.is_null(){
            d::card().show(ui,|ui|{ui.label(RichText::new("Leyendo tus ajustes…").size(16.));d::muted(ui,"Se leen de la partida guardada de Rocket League, sin abrir el juego.");
                if !self.busy&&ui.button("Volver a leer").clicked(){self.settings_loading=false;}});
            return;
        }
        let changes=self.setting_changes();
        egui::ScrollArea::vertical().id_salt("my-settings").show(ui,|ui|{
            let width=ui.available_width().min(830.);ui.set_max_width(width);
            d::card().show(ui,|ui|{
                match self.settings_tab{
                    0=>{for row in CAMERA{self.slider_row(ui,row);}
                        ui.add_space(6.);
                        for(key,label)in[("camera.shake","Vibración de cámara"),("camera.invert_swivel","Invertir giro vertical")]{
                            let mut on=self.settings_draft[key].as_bool().unwrap_or(false);if ui.checkbox(&mut on,label).changed(){self.settings_draft[key]=json!(on);}
                        }}
                    1=>{for row in CONTROLS{self.slider_row(ui,row);}
                        d::muted(ui,"Los botones del mando se cambiarán aquí en una próxima versión.");}
                    _=>{
                        ui.horizontal(|ui|{
                            ui.allocate_ui_with_layout(Vec2::new(220.,28.),egui::Layout::left_to_right(egui::Align::Center),|ui|{ui.set_min_width(220.);ui.label(RichText::new("Resolución").size(15.))});
                            let mut w=ini_int(&self.settings_draft["video.res_x"]);let mut h=ini_int(&self.settings_draft["video.res_y"]);
                            if ui.add(egui::DragValue::new(&mut w).range(640..=7680)).changed(){self.settings_draft["video.res_x"]=json!(w.to_string());}
                            ui.label("×");
                            if ui.add(egui::DragValue::new(&mut h).range(360..=4320)).changed(){self.settings_draft["video.res_y"]=json!(h.to_string());}
                        });
                        ui.horizontal(|ui|{
                            ui.allocate_ui_with_layout(Vec2::new(220.,28.),egui::Layout::left_to_right(egui::Align::Center),|ui|{ui.set_min_width(220.);ui.label(RichText::new("Modo de pantalla").size(15.))});
                            let full=ini_bool(&self.settings_draft["video.fullscreen"]);let borderless=ini_bool(&self.settings_draft["video.borderless"]);
                            let mode=if full{0}else if borderless{1}else{2};
                            for(i,label)in["Pantalla completa","Sin bordes","Ventana"].iter().enumerate(){
                                if ui.selectable_label(mode==i,*label).clicked(){self.settings_draft["video.fullscreen"]=json!(if i==0{"True"}else{"False"});self.settings_draft["video.borderless"]=json!(if i==1{"True"}else{"False"});}
                            }
                        });
                        ui.horizontal(|ui|{
                            ui.allocate_ui_with_layout(Vec2::new(220.,28.),egui::Layout::left_to_right(egui::Align::Center),|ui|{ui.set_min_width(220.);ui.label(RichText::new("FPS máximos").size(15.))});
                            let current=self.settings_draft["video.max_fps"].as_i64().unwrap_or(0);
                            egui::ComboBox::from_id_salt("max-fps").selected_text(current.to_string()).show_ui(ui,|ui|{for fps in FPS{if ui.selectable_label(current==fps,fps.to_string()).clicked(){self.settings_draft["video.max_fps"]=json!(fps);}}});
                        });
                        ui.add_space(6.);
                        let mut vsync=ini_bool(&self.settings_draft["video.vsync"]);if ui.checkbox(&mut vsync,"Sincronización vertical (VSync)").changed(){self.settings_draft["video.vsync"]=json!(if vsync{"True"}else{"False"});}
                        for(key,label)in[("video.weather","Efectos del tiempo"),("video.light_shafts","Rayos de luz"),("video.lens_flares","Destellos de lente")]{
                            let mut on=self.settings_draft[key].as_bool().unwrap_or(false);if ui.checkbox(&mut on,label).changed(){self.settings_draft[key]=json!(on);}
                        }
                    }
                }
            });
            ui.add_space(16.);
            ui.horizontal_wrapped(|ui|{
                let can=!changes.is_empty()&&!self.busy&&!self.running;
                let save=ui.add_enabled(can,d::primary(&if changes.is_empty(){"Guardar".to_owned()}else{format!("Guardar {} cambio{}",changes.len(),if changes.len()==1{""}else{"s"})}));
                let save=if self.running{save.on_disabled_hover_text("Cierra Rocket League: el juego reescribe sus ajustes al salir.")}else{save};
                if save.clicked(){
                    let mut out=serde_json::Map::new();
                    for(k,v)in changes.iter(){out.insert(k.clone(),if k=="video.res_x"||k=="video.res_y"{json!(ini_int(v))}else if k.starts_with("video.")&&v.is_string(){json!(ini_bool(v))}else{v.clone()});}
                    self.call("game_settings_write",json!({"action":"game_settings_write","changes":out}));
                }
                if ui.add_enabled(!changes.is_empty()&&!self.busy,egui::Button::new(RichText::new("Descartar").color(d::MUTED())).frame(false)).clicked(){self.settings_draft=self.game_settings["values"].clone();}
                if ui.add_enabled(!self.busy,egui::Button::new(RichText::new("Volver a leer").color(d::MUTED())).frame(false)).clicked(){self.game_settings=Value::Null;self.settings_loading=false;}
            });
            ui.add_space(6.);
            d::muted(ui,"Se guarda en tu partida guardada con el juego cerrado. Antes se hace una copia en data/settings_backups.");
        });
    }

    pub(super) fn launcher_page(&mut self,ui:&mut egui::Ui){
        if self.launcher_state.is_null()&&!self.launcher_loading&&!self.busy{self.launcher_loading=true;self.call("launcher_status",json!({"action":"launcher_status","game":self.game}));}
        egui::ScrollArea::vertical().id_salt("launcher").show(ui,|ui|{
            ui.set_max_width(ui.available_width().min(830.));
            d::card().inner_margin(24).show(ui,|ui|{
                ui.label(RichText::new("Rocket League").size(22.).strong());
                d::muted(ui,match&self.game{Some(p)=>p.display().to_string(),None=>"Elige la carpeta del juego en Ajustes.".into()});
                ui.add_space(14.);
                ui.horizontal(|ui|{
                    let can=self.game.is_some()&&!self.busy&&!self.running;
                    if ui.add_enabled(can,d::primary("  Jugar  ").min_size(Vec2::new(140.,44.))).on_disabled_hover_text(if self.running{"Rocket League ya está abierto."}else{"Elige la carpeta del juego en Ajustes."}).clicked(){self.call("launcher_play",json!({"action":"launcher_play","game":self.game}));}
                    d::muted(ui,"Abre RocketLeague.exe directamente.");
                });
            });
            ui.add_space(14.);
            d::card().inner_margin(24).show(ui,|ui|{
                ui.label(RichText::new("Jugar desde Steam").size(18.).strong());
                d::muted(ui,"Añade el .exe de Rocket League a Steam como juego externo (no hace falta tenerlo en Steam). Así puedes usar Steam Input para configurar el mando y la superposición de Steam.");
                ui.add_space(10.);
                let steam=self.launcher_state["steam"]==true;let added=self.launcher_state["in_steam"]==true;
                if !steam&&!self.launcher_state.is_null(){ui.label(RichText::new("Steam no está instalado en este PC.").color(d::MUTED()));}
                ui.horizontal_wrapped(|ui|{
                    let can=steam&&self.game.is_some()&&!self.busy&&!self.running;
                    if !added&&ui.add_enabled(can,d::button("Añadir a Steam",d::TEXT())).clicked(){self.call("launcher_steam_add",json!({"action":"launcher_steam_add","game":self.game}));}
                    if ui.add_enabled(can,d::button("Jugar desde Steam",d::GREEN())).clicked(){self.call("launcher_steam_play",json!({"action":"launcher_steam_play","game":self.game}));}
                    if added{ui.horizontal(|ui|{let(r,_)=ui.allocate_exact_size(Vec2::new(10.,18.),egui::Sense::hover());ui.painter().circle_filled(r.center(),3.5,d::GREEN());ui.label(RichText::new("Ya está en tu biblioteca de Steam").size(14.).color(d::MUTED()));});}
                });
                ui.add_space(6.);
                d::muted(ui,"Para añadirlo, Steam tiene que estar cerrado del todo. Epic puede seguir pidiéndote iniciar sesión al arrancar.");
            });
        });
    }

    /// The game rewrites the save after each match: reload when its timestamp moves.
    pub(super) fn watch_save(&mut self){
        if let Some(m)=self.game_settings["save"].as_str().and_then(|p|std::fs::metadata(p).ok()).and_then(|m|m.modified().ok()){
            let moved=self.ranks_mtime.is_some_and(|t|t!=m);
            if moved&&!self.busy{self.game_settings=Value::Null;self.settings_loading=false;}
            if !moved||!self.busy{self.ranks_mtime=Some(m);}
        }
    }

    pub(super) fn my_ranks(&mut self,ui:&mut egui::Ui){
        self.ensure_game_settings();
        self.watch_save();
        ui.ctx().request_repaint_after(std::time::Duration::from_secs(5));
        let skills=self.game_settings["skills"].as_array().cloned().unwrap_or_default();
        egui::ScrollArea::vertical().id_salt("my-ranks").show(ui,|ui|{
            if skills.is_empty(){d::card().show(ui,|ui|{ui.label(RichText::new(if self.game_settings.is_null(){"Leyendo tus rangos…"}else{"Todavía no hay rangos en tu partida guardada."}).size(16.));d::muted(ui,"Juega una partida clasificatoria y cierra el juego para que aparezca aquí.");});return;}
            let gap=14.;let columns=((ui.available_width()+gap)/(250.+gap)).floor().max(1.) as usize;let width=(ui.available_width()-gap*(columns as f32-1.))/columns as f32;
            let rows:Vec<(&str,usize,i64)>=RANK_MODES.iter().filter_map(|(id,label)|skills.iter().find(|s|s["Playlist"].as_i64()==Some(*id)).map(|s|(*label,s["Tier"].as_u64().unwrap_or(0) as usize,s["MatchesPlayed"].as_i64().unwrap_or(0)))).collect();
            for chunk in rows.chunks(columns){
                ui.with_layout(egui::Layout::left_to_right(egui::Align::Min),|ui|{
                    ui.spacing_mut().item_spacing.x=gap;
                    for(label,tier,matches)in chunk{
                        ui.allocate_ui_with_layout(Vec2::new(width,0.),egui::Layout::top_down(egui::Align::Min),|ui|{d::card().show(ui,|ui|{
                            ui.set_width(width-42.);
                            ui.horizontal(|ui|{
                                let(rect,_)=ui.allocate_exact_size(Vec2::splat(56.),egui::Sense::hover());crate::ranks::rank_icon(ui,rect,*tier);
                                ui.vertical(|ui|{ui.label(RichText::new(*label).size(13.).color(d::MUTED()));ui.label(RichText::new(crate::ranks::tier_label(*tier)).size(18.).strong());ui.label(RichText::new(if *matches==1{"1 partida".to_owned()}else{format!("{matches} partidas")}).size(12.5).color(d::MUTED()));});
                            });
                        });});
                    }
                });
                ui.add_space(gap);
            }
            d::muted(ui,"Leído de tu partida guardada: se actualiza solo cada vez que el juego guarda (al acabar una partida o al cerrarlo). El MMR exacto no se guarda en tu PC; para verlo usa la pestaña Tracker web.");
            if ui.add_enabled(!self.busy,egui::Button::new(RichText::new("Actualizar").color(d::MUTED())).frame(false)).clicked(){self.game_settings=Value::Null;self.settings_loading=false;}
        });
    }
}
