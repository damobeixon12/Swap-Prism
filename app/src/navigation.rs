use super::*;
use crate::design as d;

fn row(ui:&mut egui::Ui,label:&str,selected:bool,child:bool,arrow:Option<f32>)->bool{
    ui.push_id(("navigation-row",label),|ui|{
    let(rect,response)=ui.allocate_exact_size(Vec2::new(ui.available_width(),if child{34.}else{40.}),egui::Sense::click());
    let hover=ui.ctx().animate_bool_with_time(response.id.with("hover"),response.hovered(),0.16);
    let active=ui.ctx().animate_bool_with_time(response.id.with("active"),selected,0.20);
    let fill=d::FIELD.gamma_multiply(active.max(hover*0.55));
    ui.painter().rect_filled(rect,8,fill);
    if active>0.01{ui.painter().rect_filled(egui::Rect::from_center_size(egui::pos2(rect.left()+2.,rect.center().y),Vec2::new(3.,16.*active)),2,d::TEXT.gamma_multiply(active*0.8));}
    let color=Color32::from(egui::lerp(egui::Rgba::from(d::MUTED)..=egui::Rgba::from(d::TEXT),active.max(hover)));
    let x=rect.left()+if child{17.}else{20.};
    crate::icons::paint(ui,egui::Rect::from_center_size(egui::pos2(x,rect.center().y),Vec2::splat(if child{17.}else{19.})),label,color);
    ui.painter().text(egui::pos2(x+17.,rect.center().y),egui::Align2::LEFT_CENTER,label,egui::FontId::proportional(if child{14.}else{15.}),color);
    if let Some(open)=arrow{
        let center=egui::pos2(rect.right()-15.,rect.center().y);let angle=open*std::f32::consts::FRAC_PI_2;
        let rotate=|x:f32,y:f32|center+Vec2::new(x*angle.cos()-y*angle.sin(),x*angle.sin()+y*angle.cos());
        ui.painter().add(egui::Shape::line(vec![rotate(-2.,-4.),rotate(2.,0.),rotate(-2.,4.)],egui::Stroke::new(1.4_f32,color)));
    }
    response.on_hover_text(label).clicked()
    }).inner
}

impl App{
    pub(super) fn navigation(&mut self,ui:&mut egui::Ui){
        if self.nav_last_section!=self.section{self.nav_expanded=self.section.clone();self.nav_last_section=self.section.clone();}
        ui.spacing_mut().item_spacing.y=4.;
        d::eyebrow(ui,"MENÚ",d::MUTED);
        if row(ui,"Inicio",self.section=="Home",false,None){self.go_to("Home");}
        let query=self.navigation_search.to_lowercase();
        for(key,label)in d::MAIN_NAV{
            if !query.is_empty()&&!label.to_lowercase().contains(&query){continue;}
            let name=match key{"Skins"=>"Skins","Texturas"=>"Texturas","Fake"=>"Perfil y títulos","Overlay"=>"Overlay","Tracker"=>"Tracker","Studio"=>"Personalización","Workshop"=>"Workshop","Queue"=>"Cambios realizados","Community"=>"Discord",_=>"Ajustes"};
            let children:Vec<(&str,bool)>=match key{
                "Skins"=>SLOTS.iter().map(|(slot,name)|(*name,self.view==*slot)).collect(),
                "Texturas"=>[("Boostmeter","Boostmeter"),("Pelota","Pelota"),("Decal","Decals")].iter().map(|(k,l)|(*l,self.texture_category==*k)).collect(),
                "Fake"=>d::FAKE_TABS.iter().enumerate().map(|(i,l)|(*l,self.fake_tab==i)).collect(),
                "Overlay"=>d::OVERLAY_TABS.iter().enumerate().map(|(i,l)|(*l,self.overlay_tab==i)).collect(),
                "Studio"=>vec![("Paleta de colores",self.studio_tab==0),("Sonido del reset",self.studio_tab==1&&self.studio_sound_kind=="reset"),("Poste y travesaño",self.studio_tab==1&&self.studio_sound_kind=="post"),("Color del reset",self.studio_tab==2),("Inicio de temporada",self.studio_tab==3)],
                _=>vec![]
            };
            let open=self.nav_expanded==key&&self.section==key;
            let p=ui.ctx().animate_bool_with_time_and_easing(egui::Id::new(("navigation-spring",key)),open,0.30,egui::emath::easing::cubic_in_out);
            if row(ui,name,self.section==key,false,if children.is_empty(){None}else{Some(p)}){
                if self.section==key&&!children.is_empty(){self.nav_expanded=if open{String::new()}else{key.into()};}else{self.go_to(key);self.nav_expanded=key.into();self.nav_last_section=key.into();}
            }
            if !children.is_empty()&&p>0.001{
                let height=children.len()as f32*38.+12.;
                let(rect,_)=ui.allocate_exact_size(Vec2::new(ui.available_width(),height*p),egui::Sense::hover());
                ui.painter().line_segment([rect.left_top()+Vec2::new(18.,5.),rect.left_bottom()+Vec2::new(18.,-5.)],egui::Stroke::new(1_f32,d::MUTED.gamma_multiply(p*0.18)));
                let body=egui::Rect::from_min_size(rect.min+Vec2::new(29.,6.-(1.-p)*8.),Vec2::new(rect.width()-29.,height));
                ui.scope_builder(egui::UiBuilder::new().id_salt(("nav-children",key)).max_rect(body).layout(egui::Layout::top_down(egui::Align::Min)),|ui|{
                    ui.set_clip_rect(rect.intersect(ui.clip_rect()));ui.multiply_opacity(p);if p<0.9{ui.disable();}
                    for(i,(label,selected))in children.iter().enumerate(){if row(ui,label,*selected,true,None){match key{
                        "Skins"=>self.select_view(SLOTS[i].0),
                        "Texturas"=>{self.texture_category=["Boostmeter","Pelota","Decal"][i].into();self.texture_target=None;self.selected_texture_name.clear();},
                        "Fake"=>self.fake_tab=i,
                        "Overlay"=>self.overlay_tab=i,
                        "Studio"=>match i{0=>self.studio_tab=0,1|2=>{self.studio_tab=1;self.sound_browser=1;self.studio_sound_kind=if i==1{"reset"}else{"post"}.into();},3=>self.studio_tab=2,_=>self.studio_tab=3},
                        _=>{}
                    }}}
                });
            }
        }
    }
}
