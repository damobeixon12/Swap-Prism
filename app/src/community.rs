use super::*;
use crate::design as d;

impl App{
    pub(super) fn community(&mut self,ui:&mut egui::Ui){
        d::card().inner_margin(28).show(ui,|ui|{
            d::eyebrow(ui,"COMUNIDAD",d::MUTED());
            ui.horizontal(|ui|{ui.add(egui::Image::new(egui::include_image!("../assets/discord.png")).fit_to_exact_size(Vec2::splat(30.)));ui.heading("Swap Prism en Discord");});ui.add_space(10.);
            d::muted(ui,"Novedades, ayuda y un lugar para compartir tus personalizaciones.");ui.add_space(18.);
            ui.hyperlink_to(RichText::new("Unirme al Discord").size(19.).strong(),"https://discord.gg/wdZZGsGfq");
            ui.add_space(8.);d::muted(ui,"Se abre en tu navegador. Tú decides si quieres unirte.");
        });
    }
}
