//! Lucide monochrome icons. See assets/icons/LICENSE (ISC/MIT).
use eframe::egui::{self,Color32,Rect};
pub fn paint(ui:&egui::Ui,rect:Rect,key:&str,color:Color32){
    let (id,bytes):(&str,&'static [u8])=match key.to_lowercase().as_str(){
        "skins"|"body"|"coche"=> ("car-front",include_bytes!("../assets/icons/car-front.png")),
        "sonido del reset"|"poste y travesaño"=> ("volume-2",include_bytes!("../assets/icons/volume-2.png")),
        "color del reset"|"paleta de colores"=> ("palette",include_bytes!("../assets/icons/palette.png")),
        "inicio de temporada"=> ("frame",include_bytes!("../assets/icons/frame.png")),
        "workshop"=> ("flag",include_bytes!("../assets/icons/flag.png")),
        "texturas"|"decal"|"decals"=> ("layers",include_bytes!("../assets/icons/layers.png")),
        "fake"|"foto de perfil y nombre"|"perfil y títulos"=> ("user-round",include_bytes!("../assets/icons/user-round.png")),
        "overlay"|"fps y ping"|"boostmeter"=> ("gauge",include_bytes!("../assets/icons/gauge.png")),
        "studio"|"personalización"|"paint finish"|"acabados"=> ("palette",include_bytes!("../assets/icons/palette.png")),
        "queue"|"cambios realizados"=> ("history",include_bytes!("../assets/icons/history.png")),
        "settings"|"ajustes"=> ("settings-2",include_bytes!("../assets/icons/settings-2.png")),
        "home"|"inicio"=> ("house",include_bytes!("../assets/icons/house.png")),
        "wheels"|"wheel"=> ("circle-dot",include_bytes!("../assets/icons/circle-dot.png")),
        "boost"|"rocket boost"=> ("flame",include_bytes!("../assets/icons/flame.png")),
        "trail"|"estelas"=> ("wind",include_bytes!("../assets/icons/wind.png")),
        "goal explosion"|"explosiones"=> ("party-popper",include_bytes!("../assets/icons/party-popper.png")),
        "antenna"|"antena"|"antenas"=> ("flag",include_bytes!("../assets/icons/flag.png")),
        "topper"|"toppers"=> ("hat-glasses",include_bytes!("../assets/icons/hat-glasses.png")),
        "audio"|"engine audio"|"audio del motor"=> ("volume-2",include_bytes!("../assets/icons/volume-2.png")),
        "player banner"|"banners"=> ("rectangle-horizontal",include_bytes!("../assets/icons/rectangle-horizontal.png")),
        "avatar border"|"marcos"=> ("frame",include_bytes!("../assets/icons/frame.png")),
        "player anthem"|"himnos"=> ("music",include_bytes!("../assets/icons/music.png")),
        "pelota"=> ("volleyball",include_bytes!("../assets/icons/volleyball.png")),
        "títulos"|"player title"=> ("type",include_bytes!("../assets/icons/type.png")),
        "tracker"=> ("gauge",include_bytes!("../assets/icons/gauge.png")),
        "discord"=> ("user-round",include_bytes!("../assets/icons/user-round.png")),
        "rank"|"rank in game"=> ("shield",include_bytes!("../assets/icons/shield.png")),
        _=>("layers",include_bytes!("../assets/icons/layers.png")),
    };
    egui::Image::from_bytes(format!("bytes://lucide/{id}"),bytes).tint(color).paint_at(ui,rect.shrink(1.));
}
