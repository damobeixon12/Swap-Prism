use super::*;

impl App{
    pub(super) fn queue_motion(&mut self,ctx:&egui::Context){
        let fingerprint=json!([self.plans,self.profile_pending,if self.studio_dirty(){self.studio_draft.clone()}else{Value::Null}]).to_string();
        if fingerprint!=self.queue_visual_fingerprint{
            let previous:Value=serde_json::from_str(&self.queue_visual_fingerprint).unwrap_or(Value::Null);
            let new_plan=self.plans.iter().find(|(key,p)|previous[0][key.as_str()]!=**p).map(|(_,p)|s(p,"label").to_owned());
            let profile_changed=self.profile_pending.is_some()&&json!(self.profile_pending)!=previous[1];
            let studio_changed=self.studio_dirty()&&self.studio_draft!=previous[2];
            let changed=new_plan.is_some()||profile_changed||studio_changed;
            let quiet=self.queue_flights.last().map(|(time,_,_)|time.elapsed().as_millis()>350).unwrap_or(true);
            if !self.queue_visual_fingerprint.is_empty()&&changed&&quiet&&self.design_dir.is_none(){
                let origin=ctx.input(|i|i.pointer.interact_pos()).unwrap_or(ctx.screen_rect().center());
                let label=new_plan.unwrap_or_else(||if profile_changed{"Perfil y título".into()}else{"Personalización".into()});
                self.queue_flights.push((Instant::now(),origin,label));
            }
            self.queue_visual_fingerprint=fingerprint;
        }
        self.queue_flights.retain(|(time,_,_)|time.elapsed().as_secs_f32()<0.65);
        let painter=ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground,egui::Id::new("queue-flight")));
        for(time,origin,label)in &self.queue_flights{
            let t=(time.elapsed().as_secs_f32()/0.65).clamp(0.,1.);let eased=1.-(1.-t).powi(3);let end=self.apply_anchor.center();
            let mut pos=origin.lerp(end,eased);pos.y-=((t*std::f32::consts::PI).sin())*35.;
            let alpha=if t>0.8{(1.-t)/0.2}else{1.};let rect=egui::Rect::from_center_size(pos,Vec2::new(175.-t*35.,32.-t*8.));
            painter.rect_filled(rect,7,design::FIELD.gamma_multiply(alpha));
            painter.rect_stroke(rect,7,egui::Stroke::new(1_f32,design::GREEN.gamma_multiply(alpha)),egui::StrokeKind::Inside);
            let text:String=label.chars().take(22).collect();painter.text(pos,egui::Align2::CENTER_CENTER,text,egui::FontId::proportional(11.),design::TEXT.gamma_multiply(alpha));
        }
        if !self.queue_flights.is_empty(){ctx.request_repaint();}
    }
}
