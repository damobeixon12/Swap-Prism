use super::*;
use crate::design as d;

impl App {
    pub(super) fn go_to(&mut self, section: &str) {
        self.section = section.into();
        if section=="Workshop" && self.workshop_data["maps"].as_array().map(|v|v.is_empty()).unwrap_or(true) && !self.busy{self.call("workshop_catalog",json!({"action":"workshop_catalog","refresh":true}));}
        if section == "Settings"
            || (section == "Skins" && !SLOTS.iter().any(|(slot, _)| *slot == self.view))
        {
            self.select_view("Wheels");
        }
    }
    pub fn render(&mut self, ctx: &egui::Context) {
        let search_id=egui::Id::new("sidebar-search");
        if ctx.input_mut(|i|i.consume_key(egui::Modifiers::CTRL,egui::Key::K)){ctx.memory_mut(|m|m.request_focus(search_id));}
        let side=egui::SidePanel::left("main-navigation").exact_width(250.).resizable(false)
            .frame(egui::Frame::new().fill(d::SIDEBAR()).inner_margin(egui::Margin{left:16,right:16,top:20,bottom:14})).show(ctx,|ui|{
                ui.horizontal(|ui|{ui.add(egui::Image::new(egui::include_image!("../assets/logo.png")).fit_to_exact_size(Vec2::splat(28.)).corner_radius(7));ui.label(RichText::new("Swap Prism").strong().size(17.));});
                ui.add_space(18.);
                let field=ui.add_sized([ui.available_width(),38.],egui::TextEdit::singleline(&mut self.navigation_search).id(search_id).hint_text("Buscar").font(egui::FontId::proportional(14.5)).margin(Vec2::new(12.,10.)));
                let chip=egui::Rect::from_min_size(egui::pos2(field.rect.right()-52.,field.rect.center().y-10.),Vec2::new(44.,20.));
                if self.navigation_search.is_empty(){ui.painter().rect_stroke(chip,5,egui::Stroke::new(1_f32,d::LINE()),egui::StrokeKind::Inside);ui.painter().text(chip.center(),egui::Align2::CENTER_CENTER,"Ctrl K",egui::FontId::proportional(11.),d::MUTED());}
                ui.add_space(18.);
                egui::ScrollArea::vertical().id_salt("sidebar-nav").max_height((ui.available_height()-104.).max(150.)).show(ui,|ui|self.navigation(ui));
                ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT),|ui|{
                    ui.horizontal(|ui|{ui.label(RichText::new(format!("v{VERSION}")).size(12.5).color(d::MUTED()));ui.with_layout(egui::Layout::right_to_left(egui::Align::Center),|ui|ui.label(RichText::new("F8 · Mostrar / ocultar").size(12.5).color(d::MUTED())));});
                    ui.horizontal(|ui|{
                        // Discord: brand-coloured mark + label, opens the server invite.
                        let mark=ui.add(egui::Image::new(egui::include_image!("../assets/discord.png")).fit_to_exact_size(Vec2::splat(20.))).rect;
                        let link=ui.add(egui::Label::new(RichText::new("Discord").size(13.).color(d::TEXT())).sense(egui::Sense::click()));
                        if ui.interact(mark.union(link.rect),egui::Id::new("discord-link"),egui::Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand).on_hover_text("Únete al Discord de Swap Prism").clicked()||link.clicked(){ctx.open_url(egui::OpenUrl::new_tab(DISCORD));}
                        ui.add_space(10.);
                        if ui.add_enabled(!self.busy,egui::Label::new(RichText::new("Salir").size(12.5).color(d::MUTED())).sense(egui::Sense::click())).on_hover_cursor(egui::CursorIcon::PointingHand).clicked(){ctx.send_viewport_cmd(egui::ViewportCommand::Close);}
                    });
                    // Keeps the Discord row clear of the last navigation entry.
                    ui.add_space(10.);ui.separator();ui.add_space(4.);
                });
            });
        ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground,egui::Id::new("sidebar-edge"))).vline(side.response.rect.right(),side.response.rect.y_range(),egui::Stroke::new(1_f32,d::LINE()));
        self.toast(ctx);
        let home=self.section=="Home";
        if !home{egui::TopBottomPanel::top("header").frame(egui::Frame::new().fill(d::BACKGROUND()).inner_margin(egui::Margin{left:40,right:40,top:26,bottom:0})).show(ctx,|ui|{
            ui.horizontal(|ui|{
                let title=if self.section=="Community"{"Discord"}else{crate::navigation::NAV.iter().find(|(k,_)|*k==self.section).map(|(_,l)|*l).unwrap_or("Ajustes")};
                ui.label(RichText::new(title).size(30.).strong());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center),|ui|{
                    let(text,color)=if self.running{("Juego abierto",d::RED())}else{("Juego cerrado",d::GREEN())};
                    ui.label(RichText::new(text).size(14.).color(d::MUTED()));
                    let(rect,_)=ui.allocate_exact_size(Vec2::new(10.,18.),egui::Sense::hover());ui.painter().circle_filled(rect.center(),3.5,color);
                });
            });
            ui.add_space(18.);
            self.section_tabs(ui);
        });}
        let pending=self.pending_count();
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(d::BACKGROUND()).inner_margin(if home{egui::Margin{left:0,right:0,top:0,bottom:if pending>0{74}else{0}}}else{egui::Margin{left:40,right:40,top:0,bottom:if pending>0{74}else{12}}}))
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
                "Tracker" if self.tracker_tab == 0 => self.my_ranks(ui),
                "Tracker" => self.tracker_editor(ui),
                "Play" => self.home(ui),
                "MySettings" => self.my_settings(ui),
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
        if pending>0{self.apply_bar(ctx,pending);}
        self.splash(ctx);
    }

    /// Opening animation (1.4 s): the logo rises out of a soft glow, the name is revealed left to right, then all fades.
    fn splash(&self,ctx:&egui::Context){
        let t=self.splash_at.elapsed().as_secs_f32();
        if t>1.4||(self.shot.is_some()&&std::env::var_os("SP_SHOT_SPLASH").is_none())||self.design_dir.is_some(){return;}
        ctx.request_repaint();
        let screen=ctx.screen_rect();let out=1.-((t-1.1)/0.3).clamp(0.,1.);
        let painter=ctx.layer_painter(egui::LayerId::new(egui::Order::Tooltip,egui::Id::new("splash")));
        painter.rect_filled(screen,0,d::BACKGROUND().gamma_multiply(out));
        let grow=ease(t/0.65);let center=screen.center()-Vec2::new(0.,26.);
        painter.circle_filled(center,60.+50.*grow,d::GREEN().gamma_multiply(0.07*grow*out));
        painter.circle_filled(center,40.+30.*grow,d::GREEN().gamma_multiply(0.06*grow*out));
        if let Ok(egui::load::TexturePoll::Ready{texture})=egui::Image::new(egui::include_image!("../assets/logo.png")).load_for_size(ctx,Vec2::splat(96.)){
            let size=96.*(0.8+0.2*grow);let rect=egui::Rect::from_center_size(center+Vec2::new(0.,(1.-grow)*18.),Vec2::splat(size));
            painter.image(texture.id,rect,egui::Rect::from_min_max(egui::pos2(0.,0.),egui::pos2(1.,1.)),Color32::WHITE.gamma_multiply(grow*out));
        }
        let reveal=ease((t-0.35)/0.5);
        let galley=painter.layout_no_wrap("Swap Prism".into(),d::bold(32.),Color32::WHITE.gamma_multiply(out));let(w,h)=(galley.size().x,galley.size().y);
        let at=egui::pos2(screen.center().x-w/2.,center.y+66.+(1.-reveal)*8.);
        let clip=egui::Rect::from_min_size(at-Vec2::new(4.,4.),Vec2::new(w*reveal+8.,h+8.));
        painter.with_clip_rect(clip).galley(at,galley,Color32::WHITE);
        painter.line_segment([egui::pos2(at.x,at.y+h+6.),egui::pos2(at.x+w*reveal,at.y+h+6.)],egui::Stroke::new(2_f32,d::GREEN().gamma_multiply(out)));
    }

    /// Sub-pages of the current section, as underlined tabs under the title.
    fn section_tabs(&mut self,ui:&mut egui::Ui){
        match self.section.as_str(){
            "Skins"=>{
                let labels:Vec<&str>=SLOTS.iter().map(|(_,l)|*l).collect();let selected=SLOTS.iter().position(|(k,_)|*k==self.view).unwrap_or(usize::MAX);
                if let Some(i)=d::tabs(ui,&labels,selected){self.select_view(SLOTS[i].0);}
            }
            "Texturas"=>{
                const KEYS:[&str;3]=["Boostmeter","Pelota","Decal"];
                let selected=KEYS.iter().position(|k|*k==self.texture_category).unwrap_or(0);
                if let Some(i)=d::tabs(ui,&["Boostmeter","Pelota","Decals"],selected){self.texture_category=KEYS[i].into();self.texture_target=None;self.selected_texture_name.clear();}
            }
            "Fake"=>{
                const ORDER:[usize;3]=[1,0,2];
                let selected=ORDER.iter().position(|i|*i==self.fake_tab).unwrap_or(0);
                if let Some(i)=d::tabs(ui,&["Foto y nombre","Rango","Títulos"],selected){self.fake_tab=ORDER[i];}
            }
            "Overlay"=>{if let Some(i)=d::tabs(ui,&["FPS","Mi MMR"],self.overlay_tab.min(1)){self.overlay_tab=i;}}
            "MySettings"=>{if let Some(i)=d::tabs(ui,&["Cámara","Controles","Vídeo"],self.settings_tab){self.settings_tab=i;}}
            "Tracker"=>{if let Some(i)=d::tabs(ui,&["Mis rangos","Tracker web"],self.tracker_tab){self.tracker_tab=i;}}
            "Studio"=>{
                let selected=match(self.studio_tab,self.studio_sound_kind.as_str()){(0,_)=>0,(1,"reset")=>1,(1,_)=>2,(2,_)=>3,_=>4};
                if let Some(i)=d::tabs(ui,&["Paleta de colores","Sonido del reset","Poste y travesaño","Color del reset","Inicio de temporada"],selected){
                    match i{0=>self.studio_tab=0,1|2=>{self.studio_tab=1;self.sound_browser=1;self.studio_sound_kind=if i==1{"reset"}else{"post"}.into();},3=>self.studio_tab=2,_=>self.studio_tab=3}
                }
            }
            _=>{ui.add_space(10.);let y=ui.cursor().min.y;ui.painter().hline(ui.max_rect().x_range(),y,egui::Stroke::new(1_f32,d::LINE()));ui.add_space(22.);}
        }
    }

    /// Status as a floating notice: shows on change (and while working), fades after a few seconds.
    fn toast(&mut self,ctx:&egui::Context){
        if self.status!=self.toast_text{self.toast_text=self.status.clone();self.toast_at=Instant::now();}
        let age=self.toast_at.elapsed().as_secs_f32();
        let alpha=if self.busy{1.}else{(1.-(age-7.)/0.6).clamp(0.,1.)};
        if alpha<=0.||self.toast_text.is_empty(){return;}
        if alpha<1.||!self.busy{ctx.request_repaint_after(Duration::from_millis(60));}
        egui::Area::new(egui::Id::new("toast")).anchor(egui::Align2::RIGHT_BOTTOM,Vec2::new(-24.,-24.)).order(egui::Order::Foreground).interactable(false).show(ctx,|ui|{
            ui.multiply_opacity(alpha);
            egui::Frame::new().fill(d::PANEL()).corner_radius(10).inner_margin(egui::Margin::symmetric(16,10))
                .shadow(egui::Shadow{offset:[0,6],blur:20,spread:0,color:Color32::from_black_alpha(100)}).show(ui,|ui|{
                ui.set_max_width(420.);
                ui.horizontal(|ui|{
                    if self.busy{ui.spinner();}
                    ui.label(RichText::new(&self.toast_text).size(13.5).color(d::TEXT()));
                    if let Some(fraction)=self.job_fraction{ui.add(egui::ProgressBar::new(fraction).desired_width(120.).show_percentage());}
                });
            });
        });
    }

    /// Floating «N cambios sin aplicar · Descartar · Aplicar» bar.
    fn apply_bar(&mut self,ctx:&egui::Context,count:usize){
        egui::Area::new(egui::Id::new("apply-bar")).anchor(egui::Align2::CENTER_BOTTOM,Vec2::new(125.,-46.)).order(egui::Order::Foreground).show(ctx,|ui|{
            egui::Frame::new().fill(d::PANEL()).stroke(egui::Stroke::new(1_f32,d::LINE())).corner_radius(12).inner_margin(egui::Margin{left:20,right:10,top:8,bottom:8})
                .shadow(egui::Shadow{offset:[0,8],blur:24,spread:0,color:Color32::from_black_alpha(110)}).show(ui,|ui|{
                ui.horizontal(|ui|{
                    ui.label(RichText::new(if count==1{"1 cambio sin aplicar".to_owned()}else{format!("{count} cambios sin aplicar")}).size(15.));
                    ui.add_space(14.);
                    if ui.add(egui::Label::new(RichText::new("Ver").color(d::MUTED())).sense(egui::Sense::click())).on_hover_cursor(egui::CursorIcon::PointingHand).clicked(){self.go_to("Queue");}
                    ui.add_space(6.);
                    if ui.add_enabled(!self.busy,egui::Button::new(RichText::new("Descartar").color(d::MUTED())).frame(false)).clicked(){self.discard_pending();}
                    let dirty=self.studio_dirty();let can=!self.busy&&!self.running&&self.game.is_some();
                    let apply=ui.add_enabled(can,d::primary("Aplicar"));self.apply_anchor=apply.rect;
                    let apply=if self.running{apply.on_disabled_hover_text("Cierra Rocket League para aplicar.")}else if self.game.is_none(){apply.on_disabled_hover_text("Elige la carpeta del juego en Ajustes.")}else{apply};
                    if apply.clicked(){self.call("apply",json!({"action":"apply","game":self.game,"version":self.game_version,"team":if self.team==0{"Blue"}else{"Orange"},"plans":self.plans.values().collect::<Vec<_>>(),"studio_settings":if dirty{self.studio_draft.clone()}else{Value::Null},"profile_settings":self.profile_pending.clone()}));}
                });
            });
        });
    }

    fn discard_pending(&mut self){
        self.plans.clear();self.save_queue();
        if self.profile_pending.take().is_some(){self.profile_draft=self.profile_state["settings"].clone();self.save_profile_draft();let _=fs::write(self.root.join("data/profile-queue.json"),b"{}");}
        if self.studio_dirty(){self.studio_draft=self.studio_applied.clone();self.save_studio_draft();}
        self.status="Cambios pendientes descartados.".into();
    }

    /// Inicio is the launcher: a big cover, your profile and a Play button. Quick access below.
    fn home(&mut self, ui: &mut egui::Ui) {
        if self.launcher_state.is_null()&&!self.launcher_loading&&!self.busy{self.launcher_loading=true;self.call("launcher_status",json!({"action":"launcher_status","game":self.game}));}
        self.ensure_game_settings();
        egui::ScrollArea::vertical().id_salt("home-scroll").show(ui,|ui|{
            // The cover follows the picture's own proportions, so it is not zoomed into at wide windows.
            let width=ui.available_width();let screen=ui.ctx().screen_rect().height();
            let height=self.cover(ui.ctx()).map(|t|width*t.size.y/t.size.x).unwrap_or(screen*0.6).clamp(320.,(screen*0.72).max(320.));
            let(rect,_)=ui.allocate_exact_size(Vec2::new(width,height),egui::Sense::hover());
            self.hero_background(ui,rect);
            // Shade the lower part so text stays readable on any picture.
            let fade=egui::Rect::from_min_max(egui::pos2(rect.left(),rect.top()+rect.height()*0.35),rect.max);
            gradient(ui.painter(),fade,Color32::TRANSPARENT,d::BACKGROUND());
            let painter=ui.painter_at(rect);let left=rect.left()+44.;let bottom=rect.bottom()-40.;
            painter.text(egui::pos2(left,bottom-150.),egui::Align2::LEFT_BOTTOM,"ROCKET LEAGUE",egui::FontId::proportional(14.),d::GREEN());
            painter.text(egui::pos2(left,bottom-92.),egui::Align2::LEFT_BOTTOM,if self.running{"En partida"}else{"Listo para jugar"},d::bold(46.),Color32::WHITE);
            // Profile line: avatar, name, title and real rank.
            let settings=&self.profile_state["settings"];
            let name=if s(settings,"name").is_empty(){s(&self.snapshot,"player_name").to_owned()}else{s(settings,"name").to_owned()};
            let avatar=egui::Rect::from_min_size(egui::pos2(left,bottom-72.),Vec2::splat(56.));
            match settings["avatar"].as_str().filter(|p|Path::new(p).is_file()){
                Some(path)=>{egui::Image::from_uri(format!("file://{path}")).corner_radius(28).paint_at(ui,avatar);}
                None=>{painter.circle_filled(avatar.center(),28.,d::PANEL());painter.text(avatar.center(),egui::Align2::CENTER_CENTER,name.chars().next().unwrap_or('S').to_uppercase().to_string(),d::bold(22.),d::TEXT());}
            }
            painter.text(egui::pos2(avatar.right()+14.,avatar.center().y-2.),egui::Align2::LEFT_BOTTOM,if name.is_empty(){"Tu perfil"}else{&name},d::bold(20.),Color32::WHITE);
            let title=s(&settings["title"],"text");
            if !title.is_empty(){painter.text(egui::pos2(avatar.right()+14.,avatar.center().y+2.),egui::Align2::LEFT_TOP,title,egui::FontId::proportional(14.),d::GREEN());}
            let tier=self.game_settings["skills"].as_array().and_then(|a|a.iter().find(|s|s["Playlist"].as_i64()==Some(11))).and_then(|s|s["Tier"].as_u64()).unwrap_or(0) as usize;
            if tier>0{
                let x=avatar.right()+14.+painter.layout_no_wrap(name.clone(),d::bold(20.),Color32::WHITE).size().x.max(painter.layout_no_wrap(title.to_owned(),egui::FontId::proportional(14.),Color32::WHITE).size().x)+22.;
                let icon=egui::Rect::from_center_size(egui::pos2(x+18.,avatar.center().y),Vec2::splat(40.));crate::ranks::rank_icon(ui,icon,tier);
                painter.text(egui::pos2(icon.right()+6.,avatar.center().y),egui::Align2::LEFT_CENTER,format!("2v2 · {}",crate::ranks::tier_label(tier)),egui::FontId::proportional(14.),Color32::from_white_alpha(210));
            }
            // Play button, bottom right; Epic or Steam is chosen in Ajustes.
            let play=egui::Rect::from_min_size(egui::pos2(rect.right()-44.-230.,bottom-96.),Vec2::new(230.,62.));
            let can=self.game.is_some()&&!self.busy&&!self.running;
            let via=if self.config["launch_via"]=="steam"{"steam"}else{"epic"};
            let response=pill(ui,play,if self.running{"JUGANDO"}else{"JUGAR"},d::bold(24.),d::GREEN(),d::ON_ACCENT(),can);
            let response=if self.running{response.on_hover_text("Rocket League ya está abierto.")}else if self.game.is_none(){response.on_hover_text("Elige la carpeta del juego en Ajustes.")}else if self.busy{response.on_hover_text("Espera a que termine lo que está haciendo la app.")}else{response};
            if response.clicked(){self.call("launcher_play",json!({"action":"launcher_play","game":self.game,"via":via}));}
            painter.text(egui::pos2(play.center().x,play.bottom()+14.),egui::Align2::CENTER_TOP,if via=="steam"{"con Steam"}else{"con Epic Games"},egui::FontId::proportional(13.),Color32::from_white_alpha(170));
            // Change the cover picture, top right.
            let change=egui::Rect::from_min_size(egui::pos2(rect.right()-44.-150.,rect.top()+24.),Vec2::new(150.,32.));
            if pill(ui,change,"Cambiar imagen",egui::FontId::proportional(13.),Color32::from_black_alpha(120),Color32::WHITE,true).clicked(){
                if let Some(path)=rfd::FileDialog::new().add_filter("Imagen",&["png","jpg","jpeg","webp"]).pick_file(){self.config["home_image"]=json!(path);self.status=platform::save_config(&self.root,&self.config).err().unwrap_or("Imagen de Inicio guardada.".into());}
            }
            // Placed from the cover's edge: the buttons above moved the layout cursor.
            ui.scope_builder(egui::UiBuilder::new().max_rect(egui::Rect::from_min_size(egui::pos2(rect.left()+44.,rect.bottom()+28.),Vec2::new(width-88.,10000.))),|ui|self.quick_access(ui));
            ui.add_space(28.);
        });
    }

    /// The user's cover picture, cropped to fill; a night-sky gradient when there is none.
    fn cover(&self,ctx:&egui::Context)->Option<egui::load::SizedTexture>{
        let path=self.config["home_image"].as_str().filter(|p|Path::new(p).is_file())?;
        match egui::Image::from_uri(format!("file://{path}")).load_for_size(ctx,Vec2::splat(1.)){Ok(egui::load::TexturePoll::Ready{texture})=>Some(texture),_=>None}
    }

    fn hero_background(&self,ui:&egui::Ui,rect:egui::Rect){
        {
            if let Some(texture)=self.cover(ui.ctx()){
                let k=(rect.width()/texture.size.x).max(rect.height()/texture.size.y);
                let visible=Vec2::new(rect.width()/k/texture.size.x,rect.height()/k/texture.size.y);
                ui.painter().image(texture.id,rect,egui::Rect::from_min_size(egui::pos2((1.-visible.x)/2.,(1.-visible.y)/2.),visible),Color32::WHITE);
                return;
            }
        }
        gradient(ui.painter(),rect,Color32::from_rgb(10,16,34),Color32::from_rgb(38,58,82));
        // Fixed pseudo-random stars so the sky does not flicker between frames.
        let mut seed=0x2545f491u32;
        for _ in 0..90{
            seed^=seed<<13;seed^=seed>>17;seed^=seed<<5;let x=(seed%10000)as f32/10000.;
            seed^=seed<<13;seed^=seed>>17;seed^=seed<<5;let y=(seed%10000)as f32/10000.;
            ui.painter().circle_filled(egui::pos2(rect.left()+x*rect.width(),rect.top()+y*rect.height()*0.6),if seed%7==0{1.4}else{0.8},Color32::from_white_alpha(if seed%3==0{170}else{90}));
        }
    }

    /// Accesos rápidos: a carousel of large cards that slides on its own (pauses on hover); arrows and dots to move.
    fn quick_access(&mut self,ui:&mut egui::Ui){
        const CARDS:[(&str,&str,&str);8]=[
            ("Skins","Skins","Cambia cualquier objeto de tu inventario por otro"),
            ("Texturas","Texturas","Pelota, boostmeter y decals con tus imágenes"),
            ("Fake","Perfil","Foto, banner, nombre, rango y títulos"),
            ("Studio","Sonidos y colores","Reset, poste, paleta e inicio de temporada"),
            ("Overlay","Overlay","FPS y tu MMR encima del juego"),
            ("Workshop","Mapas","Mapas de Workshop para entrenar"),
            ("MySettings","Mis ajustes","Cámara, controles y vídeo sin abrir el juego"),
            ("Queue","Cambios","Lo aplicado y cómo volver al original"),
        ];
        let n=CARDS.len() as i64;
        let width=ui.available_width();
        let(head,_)=ui.allocate_exact_size(Vec2::new(width,34.),egui::Sense::hover());
        ui.painter().text(head.left_center(),egui::Align2::LEFT_CENTER,"Accesos rápidos",d::bold(18.),d::TEXT());
        for(i,(label,step))in[("›",1_i64),("‹",-1)].into_iter().enumerate(){
            let r=egui::Rect::from_center_size(egui::pos2(head.right()-17.-i as f32*44.,head.center().y),Vec2::splat(34.));
            if pill(ui,r,label,egui::FontId::proportional(20.),d::PANEL(),d::TEXT(),true).clicked(){self.cards_index+=step;self.cards_since=Instant::now();}
        }
        ui.add_space(12.);
        let gap=16.;let per=if width>980.{3.}else{2.};let w=(width-gap*(per-1.))/per;let h=178.;
        let(area,_)=ui.allocate_exact_size(Vec2::new(width,h),egui::Sense::hover());
        let focused=ui.ctx().input(|i|i.focused);
        if focused&&!ui.rect_contains_pointer(area)&&self.cards_since.elapsed().as_secs_f32()>4.5{self.cards_index+=1;self.cards_since=Instant::now();}
        ui.ctx().request_repaint_after(if focused{Duration::from_millis(16)}else{Duration::from_secs(1)});
        let pos=ui.ctx().animate_value_with_time(egui::Id::new("cards-pos"),self.cards_index as f32,0.55);
        let mut strip=ui.new_child(egui::UiBuilder::new().max_rect(area));strip.set_clip_rect(area.intersect(ui.clip_rect()));
        let first=pos.floor();
        for k in -1..=(per as i64){
            let index=(first as i64+k).rem_euclid(n) as usize;let(key,title,copy)=CARDS[index];
            let x=area.left()+(first-pos+k as f32)*(w+gap);
            let r=egui::Rect::from_min_size(egui::pos2(x,area.top()),Vec2::new(w,h));
            if !r.intersects(area){continue;}
            let response=strip.interact(r.intersect(area),egui::Id::new(("quick-card",k,index)),egui::Sense::click());
            let hover=strip.ctx().animate_bool_with_time(response.id.with("hover"),response.hovered(),0.15);
            let card=r.translate(Vec2::new(0.,-4.*hover));
            let painter=strip.painter();
            painter.rect_filled(card,16,d::PANEL());
            // Accent glow in the corner, brighter on hover.
            painter.with_clip_rect(card.shrink(1.).intersect(area)).circle_filled(card.right_top()+Vec2::new(-34.,34.),70.,d::GREEN().gamma_multiply(0.05+0.07*hover));
            painter.rect_stroke(card,16,egui::Stroke::new(1_f32,d::GREEN().gamma_multiply(0.15+0.5*hover)),egui::StrokeKind::Inside);
            let icon=egui::Rect::from_min_size(card.min+Vec2::new(22.,22.),Vec2::splat(40.));
            painter.rect_filled(icon,12,d::GREEN().gamma_multiply(0.14));
            crate::icons::paint(&strip,icon.shrink(9.),key,d::GREEN());
            painter.text(card.left_bottom()+Vec2::new(22.,-62.),egui::Align2::LEFT_BOTTOM,title,d::bold(21.),d::TEXT());
            let mut job=egui::text::LayoutJob::simple(copy.to_owned(),egui::FontId::proportional(14.),d::MUTED(),w-44.);job.wrap.max_rows=2;
            painter.galley(card.left_bottom()+Vec2::new(22.,-54.),painter.layout_job(job),d::MUTED());
            if response.on_hover_cursor(egui::CursorIcon::PointingHand).clicked(){self.go_to(key);}
        }
        ui.add_space(14.);
        // Dots: one per card, the current one stretched.
        let(dots,_)=ui.allocate_exact_size(Vec2::new(width,10.),egui::Sense::hover());
        let current=self.cards_index.rem_euclid(n);
        let start=dots.center().x-(n as f32*18.)/2.;
        for i in 0..n{
            let active=ui.ctx().animate_bool_with_time(egui::Id::new(("card-dot",i)),i==current,0.25);
            let dot=egui::Rect::from_min_size(egui::pos2(start+i as f32*18.,dots.top()+2.),Vec2::new(6.+active*10.,6.));
            if ui.interact(dot.expand(5.),egui::Id::new(("card-dot-hit",i)),egui::Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand).clicked(){self.cards_index+=i-current;self.cards_since=Instant::now();}
            ui.painter().rect_filled(dot,3.,if i==current{d::GREEN()}else{d::LINE()});
        }
        ui.add_space(20.);
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
                d::MUTED(),
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
        // Grid of tiles: picture first, name under it.
        let gap = 10.;
        let per = ((ui.available_width() + gap) / (132. + gap)).floor().max(1.) as usize;
        let tile = Vec2::new((ui.available_width() - gap * (per - 1) as f32) / per as f32, 150.);
        let lines = rows.len().div_ceil(per);
        egui::ScrollArea::vertical()
            .id_salt(id)
            .max_height(ui.available_height().max(120.))
            .show_rows(ui, tile.y, lines, |ui, range| {
                ui.spacing_mut().item_spacing = Vec2::splat(gap);
                for line in range {
                    ui.horizontal(|ui| {
                        for row in &rows[line * per..((line + 1) * per).min(rows.len())] {
                            let is_selected = selected == row["ID"].as_u64();
                            let (rect, response) = ui.allocate_exact_size(tile, egui::Sense::click());
                            let hover = ui.ctx().animate_bool_with_time(response.id.with("hover"), response.hovered(), 0.12);
                            ui.painter().rect_filled(rect, 12, if is_selected { d::GREEN().gamma_multiply(0.22) } else { d::PANEL() });
                            if hover > 0. { ui.painter().rect_filled(rect, 12, d::FIELD().gamma_multiply(hover)); }
                            if is_selected { ui.painter().rect_stroke(rect, 12, egui::Stroke::new(1.5_f32, d::GREEN()), egui::StrokeKind::Inside); }
                            let side = (tile.x - 24.).min(96.);
                            self.preview_at(ui, egui::Rect::from_center_size(egui::pos2(rect.center().x, rect.top() + 12. + side / 2.), Vec2::splat(side)), row);
                            let mut job = egui::text::LayoutJob::simple(s(row, "Product").to_owned(), egui::FontId::proportional(13.5), d::TEXT(), tile.x - 16.);
                            job.wrap.max_rows = 2;
                            job.halign = egui::Align::Center;
                            let galley = ui.painter().layout_job(job);
                            ui.painter().galley(egui::pos2(rect.center().x, rect.top() + 18. + side), galley, d::TEXT());
                            if id == "owned" && equipped == row["ID"].as_u64() {
                                ui.painter().circle_filled(rect.right_top() + Vec2::new(-12., 12.), 4., d::GREEN());
                            }
                            if response.clicked() { choice = Some(row.clone()); }
                            if self.design_dir.is_none() {
                                let note = format!("{}  ·  #{}{}{}", s(row, "Product"), row["ID"], if id == "owned" && equipped == row["ID"].as_u64() { "  ·  Equipado" } else { "" }, if s(row, "AssetPackage").is_empty() { "  ·  Solo consulta" } else { "" });
                                response.on_hover_text(note);
                            }
                        }
                    });
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
                ui.vertical(|ui|{d::eyebrow(ui,"COLOR",d::MUTED());ui.add_enabled_ui(self.view!="Body",|ui|{egui::ComboBox::from_id_salt("color").width(110.).selected_text(&self.color).show_ui(ui,|ui|{for c in &self.colors{ui.selectable_value(&mut self.color,c.clone(),c);}});});});
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center),|ui|{
                    let supported=["Body","Decal","Wheels","Rocket Boost","Trail","Goal Explosion","Topper","Antenna","Player Banner","Avatar Border"].contains(&self.view.as_str());
                    let pair=self.target.as_ref().zip(self.donor.as_ref());let has_assets=pair.map(|(a,b)|!s(a,"AssetPackage").is_empty()&&!s(b,"AssetPackage").is_empty()&&(a["ID"]!=b["ID"]||self.color!="Original")).unwrap_or(false);
                    if ui.add_enabled(supported&&has_assets&&!self.busy,d::button("Añadir cambio",d::GREEN())).clicked(){let target=self.target.as_ref().unwrap();let donor=self.donor.as_ref().unwrap();self.queue_plan(s(target,"AssetPackage").into(),json!({"category":target["Slot"],"kind":"item","target_id":target["ID"],"donor_id":donor["ID"],"color":self.color,"label":format!("{} → {} · {}",s(target,"Product"),s(donor,"Product"),self.color)}));self.save_queue();self.status="Añadido. Puedes seguir eligiendo o aplicar los cambios de la cola.".into();}
                });
            });
        });
        ui.add_space(14.);
        if self.view == "Body" {
            ui.label(RichText::new("Se conserva la física del coche original. Solo se aceptan modelos visuales compatibles.").size(15.).color(d::MUTED()));
        }
        let height = (ui.available_height() - 44.).max(180.);
        ui.columns(2, |columns| {
            d::card().inner_margin(14).show(&mut columns[0], |ui| {
                ui.set_min_height(height);
                d::eyebrow(ui, "01 / TU INVENTARIO", d::CYAN());
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
                d::eyebrow(ui, "02 / NUEVA APARIENCIA", d::PURPLE());
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
                ui.painter().rect_filled(rect,12,d::BACKGROUND());
                ui.put(rect.shrink(8.),egui::Image::from_uri(format!("file://{}",s(pack,"preview"))).max_size(Vec2::new(width-16.,110.)));
                let title=ui.add_sized([width,40.],egui::Label::new(RichText::new(s(pack,"name")).strong().size(15.)).wrap());
                if title.clicked(){self.selected_texture_name=s(pack,"name").into();}
                let enabled=!self.busy&&(self.texture_category!="Decal"||self.texture_target.is_some());
                if ui.add_enabled(enabled,d::button("Añadir",d::GREEN()).min_size(Vec2::new(width,38.))).clicked(){
                    let target=self.texture_target.clone().unwrap_or(Value::Null);let key=if self.texture_category=="Decal"{s(&target,"AssetPackage").to_owned()}else if self.texture_category=="Pelota"{"GameInfo_Soccar_SF.upk".into()}else{"GFX_Hud_SF.upk".into()};
                    self.queue_plan(key,json!({"category":pack["category"],"kind":"texture","digest":pack["digest"],"target_id":target["ID"],"label":format!("{} · {}",s(pack,"category"),s(pack,"name"))}));self.selected_texture_name=s(pack,"name").into();self.save_queue();self.status="Textura añadida. Aplica los cambios cuando el juego esté cerrado.".into();
                }
            });}});ui.add_space(6.);}
        });
    }

    fn changes(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui|{crate::design::toggle(ui,&mut self.changes_history,"Mostrar historial restaurado");if ui.add_enabled(!self.busy&&self.game.is_some(),egui::Button::new("Comprobar archivos")).clicked(){self.call("verify_files",json!({"action":"verify_files","game":self.game}));}});
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
            ui.label(RichText::new("Perfil, título o rango pendientes").strong());d::muted(ui,"Se aplican junto con los demás cambios, desde la barra Aplicar de abajo.");
            if ui.button("Revisar perfil").clicked(){self.section="Fake".into();self.fake_tab=1;}
            if ui.add(d::button("Descartar selección",d::RED())).clicked(){self.profile_pending=None;self.profile_draft=self.profile_state["settings"].clone();self.save_profile_draft();let _=fs::write(self.root.join("data/profile-queue.json"),b"{}");}
        });}
        self.profile_change_card(ui);
        if self.studio_dirty(){d::card().show(ui,|ui|{ui.label(RichText::new("Personalización pendiente").strong());d::muted(ui,"Sonidos, colores o inicio. Se incluyen al pulsar Aplicar.");if ui.button("Revisar").clicked(){self.section="Studio".into();}if ui.add(d::button("Descartar pendientes",d::RED())).clicked(){self.studio_draft=self.studio_applied.clone();self.save_studio_draft();}});}

        let mut remove = None;
        let mut restore = None;
        egui::ScrollArea::vertical().id_salt("changes-list").max_height((ui.available_height()-78.).max(100.)).show(ui,|ui|{
            if !self.plans.is_empty(){d::eyebrow(ui,"PENDIENTES DE APLICAR",d::GREEN());ui.add_space(4.);}
            for(key,plan)in &self.plans{d::card().inner_margin(16).show(ui,|ui|{ui.horizontal(|ui|{
                ui.vertical(|ui|{d::eyebrow(ui,"PENDIENTE",d::GREEN());ui.label(RichText::new(s(plan,"label")).strong());});
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center),|ui|{if ui.add_enabled(!self.busy,d::button("Quitar",d::RED())).clicked(){remove=Some(key.clone());}});
            });});ui.add_space(4.);}
            if let Some(entries)=self.state.as_object(){if !entries.is_empty(){ui.add_space(10.);d::eyebrow(ui,"CAMBIOS REGISTRADOS",d::CYAN());ui.add_space(4.);}for(key,entry)in entries{
                if s(&entry["target_item"],"Slot")=="Profile"||entry["hud_display_companion"]==true{continue;}
                let inactive=s(entry,"status").starts_with("restored")||s(entry,"observed_status")=="original";if inactive&&!self.changes_history{continue;}
                let(status,color)=if inactive{("Restaurado",d::MUTED())}else if s(entry,"observed_status")=="external"{("Modificado fuera de Swap Prism",d::MUTED())}else{d::status(entry)};d::card().inner_margin(16).show(ui,|ui|{
                    ui.horizontal(|ui|{
                        self.thumb(ui,&entry["donor_item"],48.);
                        let width=(ui.available_width()-120.).max(100.);ui.allocate_ui_with_layout(Vec2::new(width,0.),egui::Layout::top_down(egui::Align::LEFT),|ui|{
                            ui.set_min_width(width);d::eyebrow(ui,status,color);ui.label(RichText::new(format!("{} → {}",s(&entry["target_item"],"Product"),s(&entry["donor_item"],"Product"))).strong());if let Some(paint)=entry["color"]["name"].as_str(){d::muted(ui,paint);}
                        });
                        let restored=inactive;if ui.add_enabled(!restored&&!self.busy&&!self.running,d::button("Revertir",d::RED())).clicked(){restore=Some(key.clone());}
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
                    d::button("Restaurar todos los originales", d::RED()),
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
            d::eyebrow(ui,"PRÓXIMAMENTE",if overlay{d::CYAN()}else{d::PURPLE()});ui.add_space(18.);
            ui.label(RichText::new(match(overlay,if overlay{self.overlay_tab}else{self.fake_tab}){
                (true,0)=>"Tus FPS, sin perder de vista la partida.",(true,1)=>"Tu insignia y tu MMR encima del juego.",(true,_)=>"Un espacio para lo que viene.",
                (false,0)=>"Tu rango visual, a tu manera.",(false,1)=>"Tu foto y tu nombre, en el mismo lugar.",_=>"Un título con tu palabra y tu color."
            }).size(23.).strong());ui.add_space(12.);
            d::muted(ui,if overlay{"Esta función todavía no está disponible. Aquí podrás configurar la información que ves durante la partida."}else{"Esta función todavía no está disponible. Aquí podrás personalizar cómo se ve tu perfil en tu cliente."});ui.add_space(18.);
            ui.add_enabled(false,egui::Button::new("En preparación"));
        });
    }

    /// Which game version the objects come from, checked every time the app opens.
    fn catalog_card(&mut self, ui: &mut egui::Ui) {
        d::card().show(ui,|ui|{
            d::eyebrow(ui,"OBJETOS",d::CYAN());ui.heading("Objetos y versión");ui.add_space(10.);
            let info=self.catalog_info.clone();
            if self.catalog_checking{d::muted(ui,"Comprobando los objetos del juego…");}
            else if let Some(e)=info["error"].as_str(){ui.label(RichText::new(format!("No se pudo comprobar: {e}")).color(d::RED()));}
            else if info.is_null(){d::muted(ui,if self.game.is_none(){"Elige la carpeta del juego para comprobar sus objetos."}else{"Pendiente de comprobar."});}
            else{
                // "++Prime+Update60.1-CL-533021" -> "Update 60.1 (CL 533021)"
                let pretty=|v:&str|match v.rsplit_once("Update").map(|(_,r)|r).and_then(|r|r.split_once("-CL-")){Some((n,cl))=>format!("Update {n} (CL {cl})"),None=>v.to_owned()};
                let installed=pretty(info["installed_version"].as_str().unwrap_or(""));
                let checked=pretty(info["game_version"].as_str().unwrap_or(""));
                let (installed,checked)=(installed.as_str(),checked.as_str());
                let extra=info["extra"].as_array().map_or(0,|a|a.len());
                let total=info["game_items"].as_u64().unwrap_or(0);
                ui.label(RichText::new(format!("Rocket League instalado: {}",if installed.is_empty(){"versión desconocida"}else{installed})).size(15.));
                ui.label(RichText::new(format!("Objetos de la versión: {}",if checked.is_empty(){"desconocida"}else{checked})).size(15.));
                ui.label(RichText::new(format!("{total} objetos en el juego · {total} en Swap Prism ({extra} sin nombre oficial)")).size(15.).color(if checked==installed{d::GREEN()}else{d::RED()}));
                let at=s(&info,"checked_at");let at=if at.len()==15{format!("el {}/{}/{} a las {}:{}",&at[6..8],&at[4..6],&at[..4],&at[9..11],&at[11..13])}else{at.to_owned()};
                d::muted(ui,format!("Comprobado {at}. Los objetos sin nombre oficial salen con su nombre interno del juego: Rocket League no guarda su nombre ni su ID en los archivos."));
            }
            ui.add_space(6.);
            if ui.add_enabled(!self.catalog_checking&&self.game.is_some(),egui::Button::new("Comprobar ahora")).clicked(){self.check_catalog(true);}
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
                d::eyebrow(ui,"ROCKET LEAGUE",d::CYAN());ui.heading("Tu juego, conectado");ui.add_space(10.);
                if let Some(path)=&self.game{d::muted(ui,path.display().to_string());}else{d::muted(ui,"No se ha encontrado el juego. Selecciona la carpeta rocketleague, que contiene Binaries y TAGame.");}
                if ui.button("Elegir carpeta del juego").clicked(){if let Some(path)=rfd::FileDialog::new().set_title("Elige rocketleague (contiene Binaries y TAGame)").pick_folder(){if platform::valid_game(&path){self.game=Some(path.clone());self.config["game_path"]=json!(path);self.status=platform::save_config(&self.root,&self.config).err().unwrap_or("Ruta guardada.".into());}else{self.status="Elige la carpeta rocketleague que contiene Binaries y TAGame.".into();}}}
            });ui.add_space(12.);
            self.catalog_card(ui);ui.add_space(12.);
            if self.launcher_state.is_null()&&!self.launcher_loading&&!self.busy{self.launcher_loading=true;self.call("launcher_status",json!({"action":"launcher_status","game":self.game}));}
            d::card().show(ui,|ui|{
                d::eyebrow(ui,"JUGAR",d::GREEN());ui.heading("Abrir Rocket League con");ui.add_space(10.);
                let mut via=if self.config["launch_via"]=="steam"{"steam".to_owned()}else{"epic".to_owned()};let before=via.clone();
                ui.horizontal(|ui|{ui.radio_value(&mut via,"epic".to_owned(),"Epic Games");ui.add_space(12.);ui.radio_value(&mut via,"steam".to_owned(),"Steam");});
                if via!=before{self.config["launch_via"]=json!(via);self.status=platform::save_config(&self.root,&self.config).err().unwrap_or("Se abrirá con ".to_owned()+if via=="steam"{"Steam."}else{"Epic Games."});}
                if via=="steam"{
                    if self.launcher_state["steam"]==false{ui.label(RichText::new("Steam no está instalado en este PC.").color(d::RED()));}
                    else if self.launcher_state["in_steam"]==true{d::muted(ui,"Ya está en tu biblioteca de Steam como juego externo.");}
                    else{
                        d::muted(ui,"Hay que añadirlo a tu biblioteca de Steam una vez. Cierra Steam del todo antes.");
                        if ui.add_enabled(!self.busy&&self.game.is_some(),d::button("Añadir a Steam",d::GREEN())).clicked(){self.call("launcher_steam_add",json!({"action":"launcher_steam_add","game":self.game}));}
                    }
                }else{d::muted(ui,"Se abre a través del launcher de Epic, como siempre.");}
            });ui.add_space(12.);
            d::card().show(ui,|ui|{
                d::eyebrow(ui,"APARIENCIA",d::GREEN());ui.heading("Tema");ui.add_space(10.);
                ui.horizontal_wrapped(|ui|{
                    let current=d::theme().name;
                    for theme in d::THEMES.iter(){
                        let(rect,response)=ui.allocate_exact_size(Vec2::new(118.,64.),egui::Sense::click());
                        let selected=theme.name==current;
                        let previous=d::theme().name;d::set_theme(theme.name);let(bg,panel,accent,line)=(d::BACKGROUND(),d::PANEL(),d::GREEN(),d::LINE());d::set_theme(previous);
                        ui.painter().rect_filled(rect,10,bg);
                        ui.painter().rect_filled(egui::Rect::from_min_size(rect.min+Vec2::new(10.,10.),Vec2::new(46.,20.)),5,panel);
                        ui.painter().circle_filled(rect.min+Vec2::new(rect.width()-20.,20.),7.,accent);
                        ui.painter().rect_stroke(rect,10,egui::Stroke::new(if selected{2_f32}else{1_f32},if selected{accent}else{line}),egui::StrokeKind::Inside);
                        ui.painter().text(rect.left_bottom()+Vec2::new(10.,-12.),egui::Align2::LEFT_CENTER,theme.name,egui::FontId::proportional(13.),d::TEXT());
                        if response.on_hover_cursor(egui::CursorIcon::PointingHand).clicked()&&!selected{
                            d::set_theme(theme.name);d::setup(ui.ctx());self.config["theme"]=json!(theme.name);
                            self.status=platform::save_config(&self.root,&self.config).err().unwrap_or("Tema guardado.".into());
                        }
                    }
                });
            });ui.add_space(12.);
            d::card().show(ui,|ui|{
                d::eyebrow(ui,"EMERGENCIA",d::RED());ui.heading("Verificar archivos");
                d::muted(ui,"Si algo no se aplica o da error: compara cada paquete con el original de Epic. No borra tus cambios ni tu selección.");ui.add_space(10.);
                ui.horizontal_wrapped(|ui|{
                    if ui.add_enabled(!self.busy&&self.game.is_some(),egui::Button::new("Comprobar archivos")).clicked(){self.call("verify_files",json!({"action":"verify_files","game":self.game}));}
                    if ui.add_enabled(!self.busy,egui::Button::new("Abrir Epic Games")).clicked(){self.call("open_epic",json!({"action":"open_epic"}));}
                });
                if let Some(list)=self.file_check["modified"].as_array(){if !list.is_empty(){
                    ui.add_space(8.);d::muted(ui,"Modificados fuera de Swap Prism. En Epic: Biblioteca → Rocket League → ⋯ → Gestionar → Verificar. Después pulsa Comprobar archivos otra vez.");
                    for name in list.iter().take(12){d::muted(ui,format!("• {}",name.as_str().unwrap_or("")));}
                    if list.len()>12{d::muted(ui,format!("… y {} más",list.len()-12));}
                }}
            });ui.add_space(12.);
            d::card().show(ui,|ui|{
                d::eyebrow(ui,"TU CUENTA",d::PURPLE());ui.heading("Inventario local");d::muted(ui,"Se lee el último guardado de Rocket League.");ui.add_space(10.);
                ui.horizontal_wrapped(|ui|{if ui.add_enabled(!self.busy,egui::Button::new("Refrescar inventario")).clicked(){self.call("bootstrap",json!({"action":"bootstrap"}));}if ui.button("Ver inventario completo").clicked(){self.view="Inventory".into();self.query.clear();}});
            });ui.add_space(12.);
            d::card().show(ui,|ui|{
                d::eyebrow(ui,"Swap Prism",d::GREEN());ui.heading(format!("Versión {VERSION}"));
                d::muted(ui,"Canal estable: damobeixon12/Swap-Prism · GitHub Releases");if s(&self.config,"update_manifest_url").is_empty(){d::muted(ui,"El canal todavía no está configurado.");}else if ui.button("Buscar actualizaciones").clicked(){self.check_updates_ui();}
                ui.add_space(12.);d::muted(ui,"F8 muestra u oculta la app. Ctrl+Q sale.");d::muted(ui,"Cierra Rocket League antes de aplicar o restaurar cambios.");
            });
        });
    }
}

fn ease(t:f32)->f32{1.-(1.-t.clamp(0.,1.)).powi(3)}

/// Rounded button painted at an exact rect with centred text; does not move the layout cursor.
fn pill(ui:&mut egui::Ui,rect:egui::Rect,text:&str,font:egui::FontId,fill:Color32,color:Color32,enabled:bool)->egui::Response{
    let response=ui.interact(rect,ui.id().with(("pill",text)),if enabled{egui::Sense::click()}else{egui::Sense::hover()});
    let hover=ui.ctx().animate_bool_with_time(response.id.with("hover"),enabled&&response.hovered(),0.12);
    let fill=if enabled{fill.gamma_multiply(1.+hover*0.15)}else{fill.gamma_multiply(0.45)};
    ui.painter().rect_filled(rect.expand(hover*2.),rect.height()/2.,fill);
    ui.painter().text(rect.center(),egui::Align2::CENTER_CENTER,text,font,if enabled{color}else{color.gamma_multiply(0.7)});
    if enabled{response.on_hover_cursor(egui::CursorIcon::PointingHand)}else{response}
}

const DISCORD:&str="https://discord.gg/wdZZGsGfq";

/// Vertical colour ramp over a rectangle.
fn gradient(painter:&egui::Painter,rect:egui::Rect,top:Color32,bottom:Color32){
    let mut mesh=egui::Mesh::default();
    mesh.colored_vertex(rect.left_top(),top);mesh.colored_vertex(rect.right_top(),top);mesh.colored_vertex(rect.right_bottom(),bottom);mesh.colored_vertex(rect.left_bottom(),bottom);
    mesh.add_triangle(0,1,2);mesh.add_triangle(0,2,3);painter.add(mesh);
}
