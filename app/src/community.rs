use super::*;
use crate::design as d;

impl App{
    pub(super) fn community(&mut self,ui:&mut egui::Ui){
        d::card().inner_margin(28).show(ui,|ui|{
            d::eyebrow(ui,"COMUNIDAD",d::MUTED());
            ui.heading("Swap Prism en Discord");ui.add_space(10.);
            d::muted(ui,"Novedades, ayuda y un lugar para compartir tus personalizaciones.");ui.add_space(18.);
            ui.hyperlink_to(RichText::new("Unirme al Discord").size(19.).strong(),"https://discord.gg/dXa24Y5HtX");
            ui.add_space(8.);d::muted(ui,"Se abre en tu navegador. Tú decides si quieres unirte.");
        });
    }
}
