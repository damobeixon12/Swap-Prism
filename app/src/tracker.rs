use super::*;
use crate::design as d;

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

    pub(super) fn sync_tracker(&mut self,ctx:&egui::Context,frame:&eframe::Frame){
        let show=self.section=="Tracker"&&self.visible&&self.design_dir.is_none()&&self.tracker_requested.is_some();
        if !show{if let Some(view)=&self.tracker_web{let _=view.set_visible(false);}return;}
        let rect=self.tracker_bounds;let scale=ctx.pixels_per_point()as f64;
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
            let _=view.set_visible(true);
        }
    }
}

#[cfg(test)]mod tests{use super::*;
 #[test]fn epic_path_preserves_final_dot(){assert!(profile_url("example.").unwrap().ends_with("/example./overview"));}
 #[test]fn epic_path_is_not_a_url_input(){assert!(profile_url("a/b?#").unwrap().contains("a%2Fb%3F%23"));assert!(profile_url("").is_err());}
}
