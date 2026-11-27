use super::*;

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

/// Per ranked playlist on another player's Tracker page: rating and rank name from the playlists section. Narrow pages list
/// "Ranked Duel 1v1 / 521 / … / Gold II Div II"; wide ones (the hidden view) a table "Ranked Duel 1v1 / Gold II Division II / 521".
const LOBBY_SCRIPT:&str=r#"(()=>{const ranks=/^(Supersonic Legend|Grand Champion I{1,3}|Champion I{1,3}|Diamond I{1,3}|Platinum I{1,3}|Gold I{1,3}|Silver I{1,3}|Bronze I{1,3}|Unranked)\b/;return [...document.body.innerText.matchAll(/Ranked (Duel 1v1|Doubles 2v2|Standard 3v3)\n((?:[^\n]*\n){0,5})/g)].map(m=>{const lines=m[2].split('\n');const mmr=lines.find(l=>/^[\d.,]+$/.test(l));const rank=lines.map(l=>(l.match(ranks)||[])[1]).find(Boolean);return mmr&&rank?[m[1].toUpperCase(),mmr,rank]:null}).filter(Boolean)})()"#;

pub fn parse_lobby(result:&str)->Vec<(i64,String,usize)>{
    let mut out:Vec<(i64,String,usize)>=Vec::new();
    for(mode,mmr,rank)in serde_json::from_str::<Vec<(String,String,String)>>(result).unwrap_or_default(){
        let id=match mode.as_str(){"DUEL 1V1"=>10,"DOUBLES 2V2"=>11,"STANDARD 3V3"=>13,_=>continue};
        if !out.iter().any(|r|r.0==id){out.push((id,mmr.replace([',','.'],""),crate::ranks::tier_index(&rank)));}
    }
    out
}

pub fn profile_url(name:&str)->Result<String,String>{
    let name=name.trim();if name.is_empty()||name.chars().count()>64||name.chars().any(|c|c.is_control()){return Err("Escribe tu nombre de Epic exacto.".into());}
    let encoded:String=name.bytes().map(|b|if b.is_ascii_alphanumeric()||b"-._~".contains(&b){(b as char).to_string()}else{format!("%{b:02X}")}).collect();
    Ok(format!("https://rocketleague.tracker.network/rocket-league/profile/epic/{encoded}/overview"))
}

/// Tracker page of another player from the Stats API PrimaryId; Steam is keyed by id, the rest by name. Bots have none.
pub fn player_url(name:&str,primary:&str)->Option<String>{
    let mut parts=primary.split('|');let platform=parts.next()?.to_ascii_lowercase();let id=parts.next().unwrap_or("");
    let (path,key)=match platform.as_str(){"epic"=>("epic",name),"steam"=>("steam",id),"ps4"|"ps5"|"psn"=>("psn",name),"xboxone"|"xbox"=>("xbl",name),"switch"=>("switch",name),_=>return None};
    profile_url(key).ok().map(|u|u.replacen("/profile/epic/",&format!("/profile/{path}/"),1))
}

impl App{
    /// Other players' MMR and rank, read from their Tracker pages in a hidden view: once when they appear and once when the match ends.
    pub(super) fn lobby_tick(&mut self,ctx:&egui::Context,frame:&eframe::Frame){
        if !(self.overlay_preferences.rank&&self.overlay_preferences.rank_lobby){return;}
        crate::live_match::watch(self.live_match.clone());
        ctx.request_repaint_after(Duration::from_secs(3));
        let (playlist,players,ended)={let m=self.live_match.lock().unwrap();(m.playlist,m.players.clone(),m.ended)};
        let me=self.tracker_name.trim().to_lowercase();
        // Diagnostics for in-game checks: what the Stats API gave and what Tracker answered.
        let debug=json!({"playlist":playlist,"players":players,"queue":self.lobby_queue,"results":*self.lobby_results.lock().unwrap(),"webview":self.lobby_web.is_some(),"current":self.lobby_current.as_ref().map(|c|c.0.clone()),"page":*self.lobby_page.lock().unwrap()}).to_string();
        if !players.is_empty()&&debug!=self.lobby_debug{let _=fs::write(self.root.join("data/lobby-debug.json"),&debug);self.lobby_debug=debug;}
        if players.is_empty()||![10,11,13].contains(&playlist){
            self.rank_board.lock().unwrap().lobby.clear();
            if players.is_empty(){self.lobby_asked.clear();self.lobby_queue.clear();let mut r=self.lobby_results.lock().unwrap();if r.len()>200{r.clear();}}
            return;
        }
        // The window is usually minimized while playing: keep ticking so lookups advance.
        ctx.request_repaint_after(Duration::from_secs(1));
        if ended!=self.lobby_ended{self.lobby_ended=ended;self.lobby_asked.clear();}
        for (name,primary,_,_) in &players{
            if self.lobby_asked.contains(name)||name.to_lowercase()==me{continue;}
            self.lobby_asked.push(name.clone());
            match player_url(name,primary){Some(url)=>self.lobby_queue.push((name.clone(),url)),None=>self.lobby_results.lock().unwrap().push((name.clone(),"—".into(),0))}
        }
        {
            let results=self.lobby_results.lock().unwrap();
            let mut board=self.rank_board.lock().unwrap();
            let mine=(board.mmr.iter().find(|m|m.0==playlist).map(|m|m.1.clone()).unwrap_or("—".into()),board.tiers.iter().find(|t|t.0==playlist).map(|t|t.1).unwrap_or(0));
            board.lobby=players.iter().map(|(n,_,team,score)|{
                let (mmr,tier)=if n.to_lowercase()==me{mine.clone()}else{results.iter().rev().find(|r|&r.0==n).map(|r|(r.1.clone(),r.2)).unwrap_or(("…".into(),0))};
                crate::overlay::LobbyRow{name:n.clone(),team:*team,score:*score,mmr,tier}
            }).collect();
        }
        let now=Instant::now();
        if let Some((name,started,asked,before))=self.lobby_current.clone(){
            let Some(view)=&self.lobby_web else{return};
            let done=self.lobby_results.lock().unwrap().iter().filter(|r|r.0==name).count()>before;
            if done{self.lobby_current=None;}
            else if now-started>Duration::from_secs(15){
                let page=self.lobby_page.clone();
                let _=view.evaluate_script_with_callback("(()=>{const t=document.body.innerText;const i=t.indexOf('PLAYLISTS');return i+' '+t.length+' '+t.slice(i,i+700)})()",move|r|{*page.lock().unwrap()=r;});
                if before==0{self.lobby_results.lock().unwrap().push((name,"—".into(),0));}self.lobby_current=None;}
            else if now-started>Duration::from_secs(5)&&now-asked>Duration::from_secs(2){
                let results=self.lobby_results.clone();
                let _=view.evaluate_script_with_callback(LOBBY_SCRIPT,move|r|{if let Some(m)=parse_lobby(&r).into_iter().find(|m|m.0==playlist){results.lock().unwrap().push((name.clone(),m.1,m.2));}});
                if let Some(c)=self.lobby_current.as_mut(){c.2=now;}
            }
            return;
        }
        if self.lobby_queue.is_empty(){return;}
        let (name,url)=self.lobby_queue.remove(0);
        if self.lobby_web.is_none(){
            use wry::{WebViewBuilderExtWindows,WebViewExtWindows};
            // Tracker only draws the ratings while the page renders, and a hidden page or one inside a minimized window is
            // frozen: background throttling is off (own data folder, as different browser arguments require) and the view
            // is moved to a tiny host window of its own.
            let context=self.lobby_context.get_or_insert_with(||wry::WebContext::new(Some(self.root.join("data/tracker-lobby"))));
            let view=wry::WebViewBuilder::new_with_web_context(context)
                .with_additional_browser_args("--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection,CalculateNativeWinOcclusion --disable-background-timer-throttling --disable-renderer-backgrounding --disable-backgrounding-occluded-windows")
                .with_bounds(wry::Rect{position:wry::dpi::PhysicalPosition::new(0,0).into(),size:wry::dpi::PhysicalSize::new(1280u32,900u32).into()})
                .with_visible(true).with_focused(false).with_url(&url)
                .with_navigation_handler(|url|url.starts_with("https://rocketleague.tracker.network/")||url.starts_with("https://tracker.gg/")||url=="about:blank")
                .with_new_window_req_handler(|_,_|wry::NewWindowResponse::Deny)
                .build_as_child(frame);
            match view{Ok(view)=>{
                // Out of the main window: a minimized parent freezes the page.
                let host=crate::platform::offscreen_host();
                if host!=0{let _=view.reparent(host);}
                self.lobby_web=Some(view);
            },Err(e)=>{let _=fs::write(self.root.join("data/lobby-debug.json"),format!("webview: {e}"));self.overlay_preferences.rank_lobby=false;return;}}
        }else if let Some(view)=&self.lobby_web{let _=view.load_url(&url);}
        let before=self.lobby_results.lock().unwrap().iter().filter(|r|r.0==name).count();
        self.lobby_current=Some((name,now,now,before));
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
        let keys:Vec<&str>=self.game_settings["scoreboard_keys"].as_array().into_iter().flatten().filter_map(|k|k.as_str()).collect();
        crate::platform::set_scoreboard_keys(&keys);
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
 #[test]fn reads_lobby_rank(){assert_eq!(parse_lobby(r#"[["DUEL 1V1","1,521","Gold II"],["DOUBLES 2V2","900","Unranked"],["DUEL 1V1","1","Bronze I"]]"#),vec![(10,"1521".into(),8),(11,"900".into(),0)]);}
 #[test]fn lobby_urls_by_platform(){
  assert!(player_url("a b","Epic|x|0").unwrap().ends_with("/profile/epic/a%20b/overview"));
  assert!(player_url("n","Steam|7656|0").unwrap().ends_with("/profile/steam/7656/overview"));
  assert!(player_url("n","PS4|1|0").unwrap().contains("/profile/psn/n/"));
  assert!(player_url("Bot","Unknown|0|0").is_none());
 }
 #[test]fn epic_path_is_not_a_url_input(){assert!(profile_url("a/b?#").unwrap().contains("a%2Fb%3F%23"));assert!(profile_url("").is_err());}
}
