use super::*;
use crate::design as d;

pub const MODES:[(i64,&str);7]=[(10,"1v1 · Duel"),(11,"2v2 · Doubles"),(13,"3v3 · Standard"),(27,"Hoops"),(28,"Rumble"),(29,"Dropshot"),(30,"Snow Day")];
const TIERS:[&str;23]=["Sin rango","Bronze I","Bronze II","Bronze III","Silver I","Silver II","Silver III","Gold I","Gold II","Gold III","Platinum I","Platinum II","Platinum III","Diamond I","Diamond II","Diamond III","Champion I","Champion II","Champion III","Grand Champion I","Grand Champion II","Grand Champion III","Supersonic Legend"];
pub fn mode_label(id:i64)->&'static str{MODES.iter().find(|(key,_)|*key==id).map(|(_,label)|*label).unwrap_or("Modalidad")}
pub fn tier_label(tier:usize)->&'static str{TIERS[tier.min(22)]}
/// Tier from an English rank name as Tracker writes it ("Gold II"); unknown names are unranked.
pub fn tier_index(name:&str)->usize{TIERS.iter().position(|t|*t==name).unwrap_or(0)}
// A deliberately local simulation scale; never used to interpret real Tracker data.
const VISUAL_MMR:[i64;23]=[0,0,100,160,220,280,340,400,460,520,580,640,700,760,840,920,1000,1100,1200,1300,1500,1700,1900];
fn visual_tier(mmr:i64)->usize{(1..23).rev().find(|&i|mmr>=VISUAL_MMR[i]).unwrap_or(1)}

pub fn rank_icon(ui:&egui::Ui,rect:egui::Rect,tier:usize){
    let bytes:&'static [u8]=match tier{
        0=>include_bytes!("../assets/ranks/0.png"),
        1=>include_bytes!("../assets/ranks/1.png"),
        2=>include_bytes!("../assets/ranks/2.png"),
        3=>include_bytes!("../assets/ranks/3.png"),
        4=>include_bytes!("../assets/ranks/4.png"),
        5=>include_bytes!("../assets/ranks/5.png"),
        6=>include_bytes!("../assets/ranks/6.png"),
        7=>include_bytes!("../assets/ranks/7.png"),
        8=>include_bytes!("../assets/ranks/8.png"),
        9=>include_bytes!("../assets/ranks/9.png"),
        10=>include_bytes!("../assets/ranks/10.png"),
        11=>include_bytes!("../assets/ranks/11.png"),
        12=>include_bytes!("../assets/ranks/12.png"),
        13=>include_bytes!("../assets/ranks/13.png"),
        14=>include_bytes!("../assets/ranks/14.png"),
        15=>include_bytes!("../assets/ranks/15.png"),
        16=>include_bytes!("../assets/ranks/16.png"),
        17=>include_bytes!("../assets/ranks/17.png"),
        18=>include_bytes!("../assets/ranks/18.png"),
        19=>include_bytes!("../assets/ranks/19.png"),
        20=>include_bytes!("../assets/ranks/20.png"),
        21=>include_bytes!("../assets/ranks/21.png"),
        22=>include_bytes!("../assets/ranks/22.png"),
        _=>include_bytes!("../assets/ranks/0.png"),
    };
    egui::Image::from_bytes(format!("bytes://native-rank/{tier}"),bytes).paint_at(ui,rect);
}

impl App{
    pub(super) fn fake_rank_editor(&mut self,ui:&mut egui::Ui){
        if !self.rank_draft.is_object()||self.rank_draft.as_object().unwrap().is_empty(){self.rank_draft=json!({"enabled":true,"playlist":11,"mmr":2450,"tier":22,"division":0,"auto":true});}
        ui.heading("Fake rank");d::muted(ui,"Cambia la apariencia de tu modalidad en el menú competitivo. Tu rango real y el emparejamiento no cambian.");
        let mut mode=self.rank_draft["playlist"].as_i64().unwrap_or(11);let mut mmr=self.rank_draft["mmr"].as_i64().unwrap_or(2450);let mut tier=self.rank_draft["tier"].as_u64().unwrap_or(22).min(22)as usize;let mut division=self.rank_draft["division"].as_i64().unwrap_or(0);let mut auto=self.rank_draft["auto"]!=false;
        egui::ScrollArea::vertical().id_salt("fake-rank-editor").show(ui,|ui|{
            d::card().show(ui,|ui|{
                ui.horizontal_wrapped(|ui|{
                    ui.vertical(|ui|{ui.label("Modalidad");egui::ComboBox::from_id_salt("fake-playlist").selected_text(MODES.iter().find(|(id,_)|*id==mode).map(|(_,l)|*l).unwrap_or("2v2 · Doubles")).show_ui(ui,|ui|{for(id,label)in MODES{ui.selectable_value(&mut mode,id,label);}});});
                    ui.vertical(|ui|{ui.label("Elo / MMR visual");ui.add(egui::DragValue::new(&mut mmr).range(0..=10000).speed(1));});
                    crate::design::toggle(ui,&mut auto,"Rango automático · escala visual");
                });
                if auto{tier=visual_tier(mmr);}
                ui.horizontal_wrapped(|ui|{
                    egui::ComboBox::from_id_salt("fake-tier").selected_text(TIERS[tier]).show_ui(ui,|ui|{for(i,label)in TIERS.iter().enumerate(){ui.horizontal(|ui|{let(rect,_)=ui.allocate_exact_size(Vec2::splat(30.),egui::Sense::hover());rank_icon(ui,rect,i);if ui.selectable_value(&mut tier,i,*label).changed(){auto=false;}});}});
                    if tier>0&&tier<22{egui::ComboBox::from_id_salt("fake-division").selected_text(format!("División {}",division+1)).show_ui(ui,|ui|{for i in 0..4{ui.selectable_value(&mut division,i,format!("División {}",i+1));}});}else{division=0;}
                });
                d::muted(ui,"Escala de simulación: 1300 → GC I, 1900 → SSL. No son los umbrales reales de todas las modalidades; también puedes elegir el rango manualmente.");
            });
            ui.add_space(16.);
            d::card().inner_margin(26).show(ui,|ui|{
                d::eyebrow(ui,"SHOWCASE · SOLO LOCAL",d::MUTED());
                ui.label(RichText::new(MODES.iter().find(|(id,_)|*id==mode).unwrap().1).size(19.));
                ui.horizontal(|ui|{let(rect,_)=ui.allocate_exact_size(Vec2::splat(94.),egui::Sense::hover());rank_icon(ui,rect,tier);ui.vertical(|ui|{ui.label(RichText::new(TIERS[tier]).size(29.).strong());ui.label(RichText::new(format!("{mmr} MMR{}",if tier>0&&tier<22{format!(" · División {}",division+1)}else{String::new()})).size(22.).color(d::GREEN()));});});
            });
            ui.add_space(16.);
            ui.horizontal_wrapped(|ui|{
                if ui.add_enabled(!self.busy,d::button("Añadir fake rank",d::GREEN())).clicked(){
                    let mut settings=self.profile_pending.clone().unwrap_or_else(||self.profile_state["settings"].clone());if !settings.is_object(){settings=json!({});}
                    settings["rank"]=json!({"enabled":true,"playlist":mode,"mmr":mmr,"tier":tier,"division":division});self.queue_profile(settings);
                }
                if ui.add_enabled(!self.busy,d::button("Quitar fake rank",d::RED())).clicked(){let mut settings=self.profile_pending.clone().unwrap_or_else(||self.profile_state["settings"].clone());if settings.is_object(){settings["rank"]=Value::Null;self.queue_profile(settings);}}
            });
            d::muted(ui,"Se añade a los cambios sin aplicar. Cierra Rocket League antes de aplicar o restaurar la apariencia.");
        });
        self.rank_draft=json!({"enabled":true,"playlist":mode,"mmr":mmr,"tier":tier,"division":division,"auto":auto});
        if self.design_dir.is_none(){let _=fs::write(self.root.join("data/rank-draft.json"),self.rank_draft.to_string());}
    }
}

#[cfg(test)]mod tests{use super::*;
 #[test]fn simulation_examples(){assert_eq!(visual_tier(1300),19);assert_eq!(visual_tier(1900),22);assert_eq!(visual_tier(2450),22);}
}
