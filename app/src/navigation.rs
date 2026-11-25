use super::*;
use crate::design as d;

/// Sidebar entries: (section key, label). Sub-pages live in tabs inside each page.
pub(super) const NAV:[(&str,&str);12]=[
    ("Home","Inicio"),("Play","Jugar"),("Skins","Skins"),("Texturas","Texturas"),("Fake","Perfil"),("Studio","Sonidos y colores"),
    ("Workshop","Mapas"),("MySettings","Mis ajustes"),("Overlay","Overlay"),("Tracker","Tracker"),("Queue","Cambios"),("Settings","Ajustes"),
];

fn row(ui:&mut egui::Ui,key:&str,label:&str,selected:bool,badge:usize)->bool{
    ui.push_id(("navigation-row",key),|ui|{
        let(rect,response)=ui.allocate_exact_size(Vec2::new(ui.available_width(),38.),egui::Sense::click());
        let hover=ui.ctx().animate_bool_with_time(response.id.with("hover"),response.hovered(),0.14);
        let active=ui.ctx().animate_bool_with_time(response.id.with("active"),selected,0.18);
        ui.painter().rect_filled(rect,8,d::PANEL().gamma_multiply(active.max(hover*0.6)));
        let color=Color32::from(egui::lerp(egui::Rgba::from(d::MUTED())..=egui::Rgba::from(d::TEXT()),active.max(hover)));
        let x=rect.left()+22.;
        crate::icons::paint(ui,egui::Rect::from_center_size(egui::pos2(x,rect.center().y),Vec2::splat(17.)),key,color);
        ui.painter().text(egui::pos2(x+20.,rect.center().y),egui::Align2::LEFT_CENTER,label,egui::FontId::proportional(15.),color);
        if badge>0{
            let c=egui::pos2(rect.right()-18.,rect.center().y);
            ui.painter().circle_filled(c,9.,d::GREEN());
            ui.painter().text(c,egui::Align2::CENTER_CENTER,badge.to_string(),egui::FontId::proportional(11.),d::ON_ACCENT());
        }
        response.on_hover_cursor(egui::CursorIcon::PointingHand).clicked()
    }).inner
}

impl App{
    pub(super) fn pending_count(&self)->usize{self.plans.len()+usize::from(self.studio_dirty())+usize::from(self.profile_pending.is_some())}

    pub(super) fn navigation(&mut self,ui:&mut egui::Ui){
        ui.spacing_mut().item_spacing.y=2.;
        let query=self.navigation_search.to_lowercase();let pending=self.pending_count();
        for(key,label)in NAV{
            if !query.is_empty()&&!label.to_lowercase().contains(&query){continue;}
            if row(ui,key,label,self.section==key,if key=="Queue"{pending}else{0}){self.go_to(key);self.navigation_search.clear();}
        }
    }
}
