use super::*;
use crate::design as d;

/// Current rating per ranked playlist, read from the visible text of the user's own Tracker page.
const MMR_SCRIPT:&str=r#"(()=>[...document.body.innerText.matchAll(/RANKED (DUEL 1V1|DOUBLES 2V2|STANDARD 3V3)\s*\n\s*Current\s*\n\s*([\d.,]+)/gi)].map(m=>[m[1].toUpperCase(),m[2]]))()"#;

pub fn parse_mmr(result:&str)->Vec<(i64,String)>{
    let mut out:Vec<(i64,String)>=Vec::new();
    for(mode,mmr)in serde_json::from_str::<Vec<(String,String)>>(result).unwrap_or_default(){
        let id=match mode.as_str(){"DUEL 1V1"=>10,"DOUBLES 2V2"=>11,"STANDARD 3V3"=>13,_=>continue};
        if !out.iter().any(|r|r.0==id){out.push((id,mmr.replace([',','.'],"")));}
    }
    out
}

pub fn profile_url(name:&str)->Result<String,String>{
    let name=name.trim();if name.is_empty()||name.chars().count()>64||name.chars().any(|c|c.is_control()){return Err("Escribe tu nombre de Epic exacto.".into());}
    let encoded:String=name.bytes().map(|b|if b.is_ascii_alphanumeric()||b"-._~".contains(&b){(b as char).to_string()}else{format!("%{b:02X}")}).collect();
    Ok(format!("https://rocketleague.tracker.network/rocket-league/profile/epic/{encoded}/overview"))
}

impl App{
    pub(super) fn tracker_editor(&mut self,ui:&mut egui::Ui){
        ui.heading("Tu rango real");d::muted(ui,"Perfil oficial de Rocket League Tracker. Independiente de tu fake rank local.");
        ui.horizontal_wrapped(|ui|{
            ui.label("Epic Games");ui.add(egui::TextEdit::singleline(&mut self.tracker_name).hint_text("Tu nombre exacto").desired_width(210.));
            if ui.button("Ver mi Tracker").clicked(){match profile_url(&self.tracker_name){Ok(url)=>{
                let _=fs::write(self.root.join("data/tracker.json"),json!({"epic":self.tracker_name.trim()}).to_string());
                self.tracker_requested=Some(url);self.tracker_error.clear();
            },Err(e)=>self.tracker_error=e}}
            if let Ok(url)=profile_url(&self.tracker_name){ui.hyperlink_to("Abrir en navegador",url);}
            if ui.button("Recargar").clicked(){if let Some(view)=&self.tracker_web{let _=view.reload();}}
        });
        d::muted(ui,"La web gestiona sus datos y comprobaciones. Swap Prism no usa su API privada ni calcula un MMR real estimado.");
        if !self.tracker_error.is_empty(){ui.label(RichText::new(&self.tracker_error).color(d::RED()));}
        let (rect,_)=ui.allocate_exact_size(Vec2::new(ui.available_width(),ui.available_height().max(160.)),egui::Sense::hover());
        self.tracker_bounds=rect;
        if self.tracker_requested.is_none(){ui.painter().text(rect.center(),egui::Align2::CENTER_CENTER,"Pulsa Ver mi Tracker para cargar tu perfil aquí.",egui::FontId::proportional(17.),d::MUTED());}
    }

    /// Keeps the rank overlay fed: tiers from the save, MMR from the Tracker page (reloaded every 2 min while playing).
    pub(super) fn rank_tick(&mut self){
        // Epic name from the game's own login log (read locally); Tracker profiles are keyed by it.
        if self.tracker_name.trim().is_empty()&&s(&self.snapshot,"source").contains("SaveDataEpic"){
            let name=s(&self.snapshot,"player_name").to_owned();
            if !name.is_empty(){let _=fs::write(self.root.join("data/tracker.json"),json!({"epic":name}).to_string());self.tracker_name=name;}
        }
        if self.tracker_requested.is_none(){if let Ok(url)=profile_url(&self.tracker_name){self.tracker_requested=Some(url);}}
        self.ensure_game_settings();self.watch_save();
        let tiers:Vec<(i64,usize)>=self.game_settings["skills"].as_array().map(|a|a.iter().filter_map(|s|Some((s["Playlist"].as_i64()?,s["Tier"].as_u64()? as usize))).collect()).unwrap_or_default();
        self.rank_board.lock().unwrap().tiers=tiers;
        let Some(view)=&self.tracker_web else{return};
        let now=std::time::Instant::now();
        if self.tracker_polled.is_none_or(|t|now-t>Duration::from_secs(20)){
            self.tracker_polled=Some(now);let board=self.rank_board.clone();
            let _=view.evaluate_script_with_callback(MMR_SCRIPT,move|r|{let rows=parse_mmr(&r);if !rows.is_empty(){board.lock().unwrap().mmr=rows;}});
        }
        // Reload the Tracker page only after a match (the game rewrites the save), never periodically mid-game.
        let saved=self.ranks_mtime.map(|t|t.duration_since(std::time::UNIX_EPOCH).map(|d|d.as_secs()).unwrap_or(0));
        if self.tracker_reloaded.is_none(){self.tracker_reloaded=Some(now);self.tracker_saved=saved;}
        if saved.is_some()&&saved!=self.tracker_saved&&!crate::platform::foreground_game(){
            self.tracker_saved=saved;
            // Tracker needs a moment to register the match; wait a bit after the save changes.
            if self.tracker_reloaded.is_some_and(|t|now-t>Duration::from_secs(45)){let _=view.reload();self.tracker_reloaded=Some(now);}
        }
    }

    pub(super) fn sync_tracker(&mut self,ctx:&egui::Context,frame:&eframe::Frame){
        let show=self.section=="Tracker"&&self.tracker_tab==1&&self.visible&&self.design_dir.is_none()&&self.tracker_requested.is_some();
        let background=self.overlay_preferences.rank&&self.tracker_requested.is_some()&&self.design_dir.is_none();
        if !show{if let Some(view)=&self.tracker_web{let _=view.set_visible(false);}if !background||self.tracker_web.is_some(){return;}}
        let rect=if self.tracker_bounds.is_finite(){self.tracker_bounds}else{egui::Rect::from_min_size(egui::Pos2::ZERO,Vec2::new(900.,700.))};let scale=ctx.pixels_per_point()as f64;
        let bounds=wry::Rect{position:wry::dpi::PhysicalPosition::new((rect.left()as f64*scale)as i32,(rect.top()as f64*scale)as i32).into(),size:wry::dpi::PhysicalSize::new((rect.width()as f64*scale).max(1.)as u32,(rect.height()as f64*scale).max(1.)as u32).into()};
        let url=self.tracker_requested.as_ref().unwrap().clone();
        if self.tracker_web.is_none()&&self.tracker_error.is_empty(){
            self.tracker_context=Some(wry::WebContext::new(Some(self.root.join("data/tracker-browser"))));
            let view=wry::WebViewBuilder::new_with_web_context(self.tracker_context.as_mut().unwrap())
                .with_bounds(bounds).with_background_color((16,18,22,255)).with_focused(false).with_url(&url)
                .with_navigation_handler(|url|url.starts_with("https://rocketleague.tracker.network/")||url.starts_with("https://tracker.gg/")||url=="about:blank")
                .with_new_window_req_handler(|_,_|wry::NewWindowResponse::Deny)
                .build_as_child(frame);
            match view{Ok(view)=>{self.tracker_web=Some(view);self.tracker_loaded=url.clone();},Err(e)=>self.tracker_error=format!("No se pudo abrir la vista web: {e}. Puedes usar Abrir en navegador.")}
        }
        if let Some(view)=&self.tracker_web{
            if self.tracker_loaded!=url{let _=view.load_url(&url);self.tracker_loaded=url;}
            if self.tracker_last_bounds!=Some(bounds){let _=view.set_bounds(bounds);self.tracker_last_bounds=Some(bounds);}
            let _=view.set_visible(show);
        }
    }
}

#[cfg(test)]mod tests{use super::*;
 #[test]fn epic_path_preserves_final_dot(){assert!(profile_url("example.").unwrap().ends_with("/example./overview"));}
 #[test]fn reads_tracker_ratings(){assert_eq!(parse_mmr(r#"[["DUEL 1V1","1,683"],["DOUBLES 2V2","2.668"],["HOOPS","9"],["DUEL 1V1","7"]]"#),vec![(10,"1683".into()),(11,"2668".into())]);assert!(parse_mmr("null").is_empty());}
 #[test]fn epic_path_is_not_a_url_input(){assert!(profile_url("a/b?#").unwrap().contains("a%2Fb%3F%23"));assert!(profile_url("").is_err());}
}
