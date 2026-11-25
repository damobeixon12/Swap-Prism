#![windows_subsystem = "windows"]
mod design;
mod icons;
mod fps_probe;
mod overlay;
mod platform;
mod profile;
mod titles;
mod studio;
mod workshop;
mod motion;
mod navigation;
mod media;
mod tracker;
mod community;
mod update_panel;
mod ranks;
mod qa;
mod screens;
mod settings_page;
mod telemetry;
mod update;
use eframe::egui::{self, Color32, RichText, Vec2};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::os::windows::process::CommandExt;
use std::{
    collections::BTreeMap,
    fs,
    io::{Write,Read,BufRead,BufReader},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::mpsc::{self, Receiver, Sender},
    thread,
    time::{Duration, Instant},
};
const VERSION: &str = env!("CARGO_PKG_VERSION");

const SLOTS: [(&str, &str); 13] = [
    ("Body", "Coche"),
    ("Decal", "Decals"),
    ("Wheels", "Wheels"),
    ("Rocket Boost", "Boost"),
    ("Trail", "Estelas"),
    ("Goal Explosion", "Explosiones"),
    ("Topper", "Toppers"),
    ("Antenna", "Antenas"),
    ("Paint Finish", "Acabados"),
    ("Engine Audio", "Audio del motor"),
    ("Player Banner", "Banners"),
    ("Avatar Border", "Marcos"),
    ("Player Anthem", "Himnos"),
];
fn s<'a>(v: &'a Value, k: &str) -> &'a str {
    v[k].as_str().unwrap_or("")
}
fn read_json(path: &Path) -> Value {
    fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or(json!({}))
}
fn worker(root: &Path, request: &Value) -> Result<Value, String> {
    worker_progress(root,request,|_|{})
}
fn worker_progress(root: &Path, request: &Value, progress:impl Fn(Value)) -> Result<Value,String> {
    let mut child = Command::new(root.join("_engine/SWAPPRISMEngine.exe"))
        .env("SWAPPRISM_ROOT", root)
        .env("PYTHONIOENCODING", "utf-8")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .creation_flags(0x08000000)
        .spawn()
        .map_err(|e| format!("No se puede iniciar el motor: {e}"))?;
    child
        .stdin
        .take()
        .ok_or("Canal del motor no disponible")?
        .write_all(format!("{request}\n").as_bytes())
        .map_err(|e| e.to_string())?;
    let mut stderr=child.stderr.take().ok_or("Canal de diagnóstico no disponible")?;
    let errors=thread::spawn(move||{let mut bytes=Vec::new();let _=stderr.read_to_end(&mut bytes);bytes});
    let stdout=child.stdout.take().ok_or("Canal de respuesta no disponible")?;
    let mut result=None;
    for line in BufReader::new(stdout).lines(){
        let line=line.map_err(|e|e.to_string())?;
        if let Ok(frame)=serde_json::from_str::<Value>(&line){
            if frame["progress"].is_object(){progress(frame["progress"].clone());}
            else if frame["ok"].is_boolean(){result=Some(frame);}
        }
    }
    let _=child.wait().map_err(|e|e.to_string())?;
    let diagnostics=errors.join().unwrap_or_default();
    let result=result.ok_or_else(||format!("El motor terminó sin respuesta. {}",String::from_utf8_lossy(&diagnostics).chars().take(1000).collect::<String>()))?;
    if result["ok"] == true {
        Ok(result["result"].clone())
    } else {
        Err(s(&result, "error").into())
    }
}
enum Event {
    Progress(Value),
    Engine(String, Result<Value, String>),
    Update(Result<Option<update::Release>, String>),
    Downloaded(Result<PathBuf, String>),
    UpdateBytes(u64,Option<u64>),
    Presence(bool),
    Toggle,
    Located(Option<(PathBuf, String)>),
}
struct App {
    rank_draft:Value,
    tracker_name:String,tracker_requested:Option<String>,tracker_loaded:String,tracker_error:String,
    tracker_web:Option<wry::WebView>,tracker_context:Option<wry::WebContext>,tracker_bounds:egui::Rect,tracker_last_bounds:Option<wry::Rect>,
    media_data:Value,media_source:String,media_query:String,media_url:String,
    texture_shop:bool,texture_source:usize,sound_browser:usize,audio_library:Vec<Value>,
    page_motion_key:String,
    page_motion_start:Instant,
    studio_sound_kind:String,
    workshop_data: Value,
    workshop_query: String,
    workshop_sort: usize,
    workshop_downloaded: bool,
    queue_visual_fingerprint: String,
    queue_flights: Vec<(Instant,egui::Pos2,String)>,
    apply_anchor: egui::Rect,
    root: PathBuf,
    config: Value,
    rows: Vec<Value>,
    snapshot: Value,
    packs: Vec<Value>,
    state: Value,
    plans: BTreeMap<String, Value>,
    view: String,
    team: usize,
    query: String,
    owned_query: String,
    target: Option<Value>,
    donor: Option<Value>,
    color: String,
    colors: Vec<String>,
    texture_category: String,
    texture_target: Option<Value>,
    status: String,
    toast_text: String,
    toast_at: Instant,
    busy: bool,
    job_fraction: Option<f32>,
    game: Option<PathBuf>,
    game_version: String,
    running: bool,
    tx: Sender<Event>,
    rx: Receiver<Event>,
    available_update: Option<update::Release>,
    update_file: Option<PathBuf>,
    update_sha256: String,
    update_open: bool,
    update_checking: bool,
    update_downloading: bool,
    update_bytes:u64,
    update_total:Option<u64>,
    update_error:String,
    update_waiting:Option<Instant>,
    visible: bool,
    started: Instant,
    smoke: Option<PathBuf>,
    // Dev check: `--screenshot <section> <file.png>` saves one frame and exits.
    shot: Option<(PathBuf,u32)>,
    first_frame: bool,
    section: String,
    feature_page: usize,
    feature_changed: Instant,
    fake_tab: usize,
    profile_draft: Value,
    profile_state: Value,
    profile_pending: Option<Value>,
    title_draft: Value,
    studio_draft: Value,
    studio_applied: Value,
    studio_tab: usize,
    studio_palette: String,
    studio_slot: usize,
    title_page: usize,
    title_query: String,
    navigation_search: String,
    changes_history: bool,
    file_check: Value,
    game_settings: Value,
    settings_draft: Value,
    settings_tab: usize,
    settings_loading: bool,
    ranks_mtime: Option<std::time::SystemTime>,
    launcher_state: Value,
    launcher_loading: bool,
    tracker_tab: usize,
    audio_preview: Option<(String,Instant,f32)>,
    overlay_tab: usize,
    pack_query: String,
    selected_texture_name: String,
    design_dir: Option<PathBuf>,
    design_index: usize,
    design_wait: u32,
    design_requested: bool,
    telemetry: telemetry::Controller,
    overlay_preferences: overlay::Preferences,
    overlay_placement: std::sync::Arc<std::sync::Mutex<overlay::Placement>>,
    rank_board: std::sync::Arc<std::sync::Mutex<overlay::RankBoard>>,
    rank_placement: std::sync::Arc<std::sync::Mutex<overlay::Placement>>,
    tracker_polled: Option<std::time::Instant>,
    tracker_reloaded: Option<std::time::Instant>,
}
impl App {
    fn new(cc: &eframe::CreationContext<'_>, root: PathBuf, smoke: Option<PathBuf>) -> Self {
        let started = Instant::now();
        egui_extras::install_image_loaders(&cc.egui_ctx);
        let config = platform::config(&root);
        design::set_theme(config["theme"].as_str().unwrap_or(""));
        design::setup(&cc.egui_ctx);
        let rows =
            serde_json::from_slice(include_bytes!("../assets/catalog.json")).unwrap_or_default();
        let plans =
            serde_json::from_value(read_json(&root.join("data/queue.json"))).unwrap_or_default();
        let overlay_preferences: overlay::Preferences =
            serde_json::from_value(config["overlay"].clone()).unwrap_or_default();
        let telemetry =
            telemetry::Controller::new(root.clone(), cc.egui_ctx.clone(), overlay_preferences.fps);
        let (tx, rx) = mpsc::channel();
        let app = Self {
            rank_draft:read_json(&root.join("data/rank-draft.json")),
            tracker_name:s(&read_json(&root.join("data/tracker.json")),"epic").to_owned(),tracker_requested:None,tracker_loaded:String::new(),tracker_error:String::new(),tracker_web:None,tracker_context:None,tracker_bounds:egui::Rect::NOTHING,tracker_last_bounds:None,
            media_data:json!({}),media_source:String::new(),media_query:String::new(),media_url:String::new(),texture_shop:false,texture_source:0,sound_browser:1,audio_library:vec![],
            page_motion_key:String::new(),page_motion_start:Instant::now(),studio_sound_kind:"reset".into(),
            workshop_data: {
                let mut data=read_json(&root.join("data/workshop/catalog.json"));
                let library=read_json(&root.join("data/workshop/library.json"));
                if !data["maps"].is_array(){data=json!({"maps":[]});}
                if let Some(maps)=data["maps"].as_array_mut(){for item in maps {let key=s(item,"key").to_owned();if library[&key].is_object(){item["local"]=library[&key].clone();}let image=root.join(format!("data/workshop/images/{key}.png"));if image.is_file(){item["preview"]=json!(image);}}}
                let known:Vec<String>=data["maps"].as_array().unwrap().iter().map(|m|s(m,"key").to_owned()).collect();
                if let Some(entries)=library.as_object(){for(key,entry)in entries{if !known.contains(key){let mut item=entry.clone();item["key"]=json!(key);item["local"]=entry.clone();data["maps"].as_array_mut().unwrap().push(item);}}}
                data
            },
            workshop_query:String::new(),workshop_sort:0,workshop_downloaded:false,
            queue_visual_fingerprint:String::new(),queue_flights:vec![],apply_anchor:egui::Rect::NOTHING,
            root: root.clone(),
            config,
            rows,
            snapshot: json!({}),
            packs: vec![],
            state: read_json(&root.join("data/swaps.json")),
            plans,
            view: "Wheels".into(),
            team: 0,
            query: String::new(),
            owned_query: String::new(),
            target: None,
            donor: None,
            color: "Original".into(),
            colors: vec!["Original".into()],
            texture_category: "Boostmeter".into(),
            texture_target: None,
            status: "Leyendo inventario en segundo plano…".into(),
            toast_text: String::new(),
            toast_at: Instant::now(),
            busy: true,
            job_fraction: None,
            game: None,
            game_version: String::new(),
            running: false,
            tx: tx.clone(),
            rx,
            available_update: None,
            update_file: None,
            update_sha256: String::new(),
            update_open:false,update_checking:false,update_downloading:false,update_bytes:0,update_total:None,update_error:String::new(),update_waiting:None,
            visible: true,
            started,
            smoke,
            shot: None,
            first_frame: true,
            section: "Home".into(),
            feature_page: 0,
            feature_changed: Instant::now(),
            fake_tab: 0,
            profile_draft: read_json(&root.join("data/profile-draft.json")),
            profile_state: json!({}),
            profile_pending: {let value=read_json(&root.join("data/profile-queue.json"));value.as_object().filter(|m|!m.is_empty()).map(|_|value.clone())},
            title_draft: read_json(&root.join("data/title-draft.json")),
            studio_draft: {let draft=read_json(&root.join("data/studio-draft.json"));if draft.as_object().map(|m|!m.is_empty()).unwrap_or(false){draft}else{read_json(&root.join("data/studio.json"))}},
            studio_applied: read_json(&root.join("data/studio.json")),
            studio_tab: 0,
            studio_palette: "blue".into(),
            studio_slot: 0,
            title_page: 0,
            title_query: String::new(),
            navigation_search: String::new(),
            changes_history: false,
            file_check: Value::Null,
            game_settings: Value::Null,
            settings_draft: Value::Null,
            settings_tab: 0,
            settings_loading: false,
            ranks_mtime: None,
            launcher_state: Value::Null,
            launcher_loading: false,
            tracker_tab: 0,
            audio_preview: None,
            overlay_tab: 0,
            pack_query: String::new(),
            selected_texture_name: String::new(),
            design_dir: std::env::args()
                .position(|a| a == "--design-check")
                .and_then(|i| std::env::args().nth(i + 1))
                .map(PathBuf::from),
            design_index: 0,
            design_wait: 0,
            design_requested: false,
            telemetry,
            overlay_preferences,
            overlay_placement: std::sync::Arc::new(std::sync::Mutex::new(
                overlay::Placement::default(),
            )),
            rank_board: Default::default(),
            rank_placement: Default::default(),
            tracker_polled: None,
            tracker_reloaded: None,
        };
        if let Some(i) = std::env::args().position(|a| a == "--overlay-window-check") {
            app.overlay_placement.lock().unwrap().capture =
                std::env::args().nth(i + 1).map(PathBuf::from);
        }
        let r = root.clone();
        let send = tx.clone();
        thread::spawn(move || {
            let _ = send.send(Event::Engine(
                "bootstrap".into(),
                worker(&r, &json!({"action":"bootstrap"})),
            ));
        });
        let saved = app.config["game_path"].as_str().map(str::to_owned);
        let send = tx.clone();
        thread::spawn(move || {
            let _ = send.send(Event::Located(platform::detect(saved.as_deref())));
        });
        let url = s(&app.config, "update_manifest_url").to_string();
        if !url.is_empty() {
            let send = tx.clone();
            thread::spawn(move || {
                let _ = send.send(Event::Update(update::check(&url, VERSION)));
            });
        }
        let paint = cc.egui_ctx.clone();
        let send = tx.clone();
        thread::spawn(move || {
            let mut visible = true;
            let mut pressed = false;
            let mut last = None;
            loop {
                let p = platform::running();
                if last != Some(p) {
                    let _ = send.send(Event::Presence(p));
                    last = Some(p)
                }
                let now = unsafe {
                    windows_sys::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState(0x77)
                } < 0;
                if now && !pressed {
                    visible = !visible;
                    paint.send_viewport_cmd(egui::ViewportCommand::Visible(visible));
                    if visible {
                        paint.send_viewport_cmd(egui::ViewportCommand::Focus);
                    }
                    let _ = send.send(Event::Toggle);
                    paint.request_repaint();
                }
                pressed = now;
                thread::sleep(Duration::from_millis(120));
            }
        });
        app
    }
    fn call(&mut self, action: &str, request: Value) {
        if self.busy {
            return;
        }
        self.busy = true;
        self.job_fraction=None;
        self.status = "Procesando en segundo plano…".into();
        let root = self.root.clone();
        let tx = self.tx.clone();
        let action = action.to_owned();
        thread::spawn(move || {
            let result=std::panic::catch_unwind(std::panic::AssertUnwindSafe(||worker_progress(&root,&request,|p|{let _=tx.send(Event::Progress(p));}))).unwrap_or_else(|_|Err("El motor se interrumpió; revisa el estado de los cambios.".into()));
            let _ = tx.send(Event::Engine(action, result));
        });
    }
    fn queue_plan(&mut self,key:String,plan:Value){
        let category=s(&plan,"category").to_owned();
        let rows=self.rows.clone();let packs=self.packs.clone();
        self.plans.retain(|_,old|{
            let derived=if s(old,"kind")=="item"{rows.iter().find(|r|r["ID"]==old["target_id"]).map(|r|s(r,"Slot")).unwrap_or("")}else{packs.iter().find(|r|r["digest"]==old["digest"]).map(|r|s(r,"category")).unwrap_or("")};
            let existing=if s(old,"category").is_empty(){derived}else{s(old,"category")};
            existing!=category
        });
        self.plans.insert(key,plan);
    }
    fn save_queue(&self) {
        let _ = fs::create_dir_all(self.root.join("data"));
        let path = self.root.join("data/queue.json.tmp");
        if fs::write(&path, serde_json::to_vec_pretty(&self.plans).unwrap()).is_ok() {
            let _ = fs::rename(path, self.root.join("data/queue.json"));
        }
    }
    fn equipped(&self, slot: &str) -> Option<Value> {
        self.snapshot["account_equipped"]
            .as_array()
            .into_iter()
            .flatten()
            .chain(
                self.snapshot["active"]["teams"][self.team]["items"]
                    .as_array()
                    .into_iter()
                    .flatten(),
            )
            .find(|v| s(v, "Slot") == slot)
            .cloned()
    }
    fn owned(&self, slot: &str) -> Vec<Value> {
        let mut entries = BTreeMap::new();
        for v in self.snapshot["inventory"].as_array().into_iter().flatten() {
            if s(v, "Slot") == slot && v["blueprint"] != true {
                entries
                    .entry(v["ID"].as_u64().unwrap_or(0))
                    .or_insert(v.clone());
            }
        }
        if let Some(v) = self.equipped(slot) {
            entries.entry(v["ID"].as_u64().unwrap_or(0)).or_insert(v);
        }
        let mut out: Vec<_> = entries.into_values().collect();
        out.sort_by_key(|v| s(v, "Product").to_lowercase());
        out
    }
    fn select_view(&mut self, view: &str) {
        self.view = view.into();
        self.target = None;
        self.donor = None;
        self.query.clear();
        self.owned_query.clear();
        self.color = "Original".into();
    }
    fn thumb(&self, ui: &mut egui::Ui, v: &Value, size: f32) {
        let url = s(v, "Image");
        let p=platform::thumbnail_path(&self.root,url);
        if !url.is_empty() && p.is_file() {
            ui.add(
                egui::Image::from_uri(format!("file://{}", p.display()))
                    .fit_to_exact_size(Vec2::splat(size)),
            );
        } else {
            if let Some(pack) = self.packs.iter().find(|p| s(p, "name") == s(v, "Product")) {
                ui.add(
                    egui::Image::from_uri(format!("file://{}", s(pack, "preview")))
                        .fit_to_exact_size(Vec2::splat(size)),
                );
            } else {
                let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), egui::Sense::hover());
                ui.painter().rect_filled(rect, 10, design::FIELD());
                ui.painter().circle_stroke(
                    rect.center(),
                    size * 0.2,
                    egui::Stroke::new(1_f32, design::MUTED()),
                );
            }
        }
    }
    fn events(&mut self, ctx: &egui::Context) {
        while let Ok(event) = self.rx.try_recv() {
            match event {
                Event::Progress(p)=>{self.status=format!("{} · {}/{}",s(&p,"message"),p["current"],p["total"]);let total=p["total"].as_f64().unwrap_or(0.);self.job_fraction=if total>0.{Some((p["current"].as_f64().unwrap_or(0.)/total) as f32)}else{None};},
                Event::Engine(action, result) => {
                    self.busy = false;
                    self.job_fraction=None;
                    if action.starts_with("game_settings"){self.settings_loading=false;}
                    if action.starts_with("launcher_"){self.launcher_loading=false;}
                    if let Err(error) = &result {
                        let _ = fs::write(self.root.join("data/last-error.txt"), error);
                    }
                    match result {
                        Err(e) => self.status = e,
                        Ok(v) => {
                            if action == "bootstrap" {
                                self.studio_applied=v["studio"].clone();
                                if self.studio_draft.as_object().map(|v|v.is_empty()).unwrap_or(true){self.studio_draft=v["studio"].clone();}
                                self.profile_state = v["profile"].clone();
                                if self.profile_active()&&v["profile"]["codec_revision"].as_u64().unwrap_or(0)<5&&self.profile_pending.is_none(){
                                    self.profile_pending=Some(v["profile"]["settings"].clone());
                                    let _=fs::write(self.root.join("data/profile-queue.json"),serde_json::to_vec_pretty(&self.profile_pending).unwrap());
                                }
                                if self.title_draft.as_object().map(|m|m.is_empty()).unwrap_or(true){
                                    self.title_draft=v["profile"]["settings"]["title"].as_object().map(|_|v["profile"]["settings"]["title"].clone()).unwrap_or(json!({"text":"SWAP PRISM","color":"#75EAA6","color2":"#68DAF9","glow":"#75EAA6","glow_enabled":true,"multicolor":false,"target":""}));
                                }
                                if !self.profile_draft.is_object() || self.profile_draft.as_object().map(|m|m.is_empty()).unwrap_or(true) {
                                    self.profile_draft = v["profile"]["settings"].clone();
                                }
                                self.snapshot = v["snapshot"].clone();
                                self.packs = v["packs"].as_array().cloned().unwrap_or_default();
                                self.audio_library=v["sounds"].as_array().cloned().unwrap_or_default();
                                self.state = v["state"].clone();
                                self.colors = v["colors"]
                                    .as_array()
                                    .into_iter()
                                    .flatten()
                                    .filter_map(|v| v.as_str().map(str::to_owned))
                                    .collect();
                                self.status = if let Some(error) = v["errors"][0].as_str() {
                                    error.to_owned()
                                } else {
                                    "Inventario actualizado. Elige el original y su apariencia."
                                        .into()
                                };
                            } else if action.starts_with("media_") {
                                if s(&v["media"],"source")==self.media_source{self.media_data=v["media"].clone();}
                                if let Some(packs)=v["packs"].as_array(){self.packs=packs.clone();}
                                if let Some(sounds)=v["sounds"].as_array(){self.audio_library=sounds.clone();}
                                self.status=if action=="media_download"{"Contenido disponible en Mi biblioteca. Elígelo para añadirlo a los cambios sin aplicar."}else if action=="media_preview"{"Preescucha terminada. No se ha añadido el sonido a tu biblioteca."}else{"Catálogo actualizado."}.into();
                            } else if action.starts_with("workshop_") {
                                self.workshop_data=v;
                                self.status=if action=="workshop_download"||action=="workshop_import"{"Mapa guardado en la biblioteca. Pulsa Elegir para añadirlo a los cambios sin aplicar."}else{"Biblioteca de mapas actualizada."}.into();
                            } else if action == "sound_import" {
                                let kind=s(&v,"kind").to_owned();
                                if !self.studio_draft["sounds"].is_object(){self.studio_draft["sounds"]=json!({});}
                                self.studio_draft["sounds"][&kind]=v;
                                self.audio_library=self.root.join("data/sounds").read_dir().into_iter().flatten().filter_map(|e|e.ok()).filter(|e|e.path().extension().and_then(|x|x.to_str())==Some("json")).map(|e|read_json(&e.path())).collect();
                                self.save_studio_draft();
                                self.status="MP3 validado. Escúchalo antes de aplicar.".into();
                            } else if action == "sound_preview" {
                                self.audio_preview=None;
                                self.status="Preescucha terminada.".into();
                            } else if action == "game_settings" || action == "game_settings_write" {
                                self.settings_draft=v["values"].clone();
                                self.status=if action=="game_settings_write"{format!("Ajustes guardados. Copia de seguridad en {}",s(&v,"backup"))}else{"Ajustes leídos de tu partida guardada.".into()};
                                self.game_settings=v;
                            } else if action.starts_with("launcher_") {
                                if v.get("steam").is_some(){self.launcher_state=v.clone();}
                                self.status=match action.as_str(){"launcher_play"=>"Iniciando Rocket League…","launcher_steam_play"=>"Iniciando Rocket League desde Steam…","launcher_steam_add"=>"Rocket League añadido a Steam.",_=>"Lanzador listo."}.into();
                            } else if action == "verify_files" {
                                self.state=v["state"].clone();self.profile_state=v["profile"].clone();
                                let healed=v["healed"].as_array().map(|a|a.len()).unwrap_or(0);let modified=v["modified"].as_array().map(|a|a.len()).unwrap_or(0);
                                self.status=if v["epic"]!=true{"No hay manifiesto de Epic (Steam): solo se comprobaron las copias de Swap Prism.".into()}else if modified>0{format!("{modified} archivo(s) modificados fuera de Swap Prism. Verifica los archivos en Epic y vuelve a comprobar.")}else if healed>0{format!("Archivos correctos. {healed} cambio(s) los había quitado Epic: vuelve a aplicarlos.")}else{"Todos los archivos están correctos.".into()};
                                self.file_check=v;
                            } else if action == "open_epic" {
                                self.status="Epic abierto: Biblioteca → Rocket League → ⋯ → Gestionar → Verificar.".into();
                            } else if action == "refresh_state" {
                                self.state=v["state"].clone();self.status="Cambios comprobados contra los archivos actuales.".into();
                            } else if action == "profile_import" {
                                let key=s(&v,"kind").to_owned();
                                self.profile_draft[&key]=v["path"].clone();
                                self.save_profile_draft();
                                self.queue_profile(self.profile_draft.clone());
                                self.status="Imagen añadida. Pulsa Aplicar abajo.".into();
                            } else if action == "import" {
                                self.packs = v["packs"].as_array().cloned().unwrap_or_default();
                                self.status = if let Some(e) = v["errors"][0].as_str() {
                                    e.to_owned()
                                } else {
                                    "Texturas importadas.".into()
                                };
                            } else {
                                if v["profile"].is_object(){self.profile_state=v["profile"].clone();}
                                if v["studio"].is_object(){self.studio_draft=v["studio"].clone();self.studio_applied=v["studio"].clone();self.save_studio_draft();}
                                self.state = v["state"].clone();
                                if action == "apply" {
                                    self.profile_pending=None;let _=fs::write(self.root.join("data/profile-queue.json"),b"{}");
                                    self.plans.clear();
                                    self.save_queue();
                                }
                                self.status = format!(
                                    "{} paquetes verificados. Ya puedes abrir Rocket League.",
                                    v["count"]
                                );
                                if v["cached"]==true{self.status="El perfil ya estaba aplicado. No hay cambios pendientes.".into();}
                                self.section = "Queue".into();
                            }
                        }
                    }
                }
                Event::Located(found) => {
                    if let Some((path, version)) = found {
                        self.game = Some(path);
                        self.game_version = version;
                    } else {
                        self.status =
                            "No se encontró Rocket League. Selecciona su carpeta en Ajustes."
                                .into();
                        self.section = "Settings".into();
                    }
                }
                Event::Presence(value) => self.running = value,
                Event::Toggle => {
                    self.visible = !self.visible;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Visible(self.visible));
                    if self.visible {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                    }
                }
                Event::UpdateBytes(bytes,total)=>{self.update_bytes=bytes;self.update_total=total;},
                Event::Update(result)=>{self.update_checking=false;match result{
                    Ok(release)=>{self.available_update=release;if self.available_update.is_some(){self.update_open=true;}self.status=if self.available_update.is_some(){"Hay una actualización disponible."}else{"Ya tienes la última versión publicada."}.into();},
                    Err(error)=>{self.update_error=error.clone();self.status=error;}
                }},
                Event::Downloaded(result) => {
                    self.busy = false;self.update_downloading=false;
                    match result {
                        Ok(path) => {
                            self.update_sha256=self.available_update.as_ref().map(|r|r.sha256.clone()).unwrap_or_default();
                            self.update_file = Some(path);
                            self.available_update = None;
                            self.start_native_update();
                        }
                        Err(e) => {self.update_error=e.clone();self.status=e;},
                    }
                }
            }
        }
    }
}
impl eframe::App for App {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0., 0., 0., 0.]
    }
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.events(ctx);
        if self.overlay_placement.lock().unwrap().captured {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        ctx.request_repaint_after(Duration::from_millis(120));
        if self.first_frame {
            self.first_frame = false;
            let _ = fs::create_dir_all(self.root.join("data"));
            let _=fs::write(self.root.join("data/startup.json"),json!({"version":VERSION,"native_first_frame_ms":self.started.elapsed().as_millis(),"catalog":self.rows.len(),"frontend":"Rust / egui"}).to_string());
        }
        if self.busy && ctx.input(|i| i.viewport().close_requested()) {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
        }
        if !self.busy && ctx.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::Q)) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        self.prepare_design(ctx);
        self.render(ctx);
        if self.overlay_preferences.rank{self.rank_tick();}
        self.sync_tracker(ctx,_frame);
        self.queue_motion(ctx);
        self.capture_design(ctx);
        overlay::show(
            ctx,
            &self.overlay_preferences,
            self.telemetry.reading.clone(),
            self.overlay_placement.clone(),
        );
        overlay::show_ranks(ctx,&self.overlay_preferences,self.rank_board.clone(),self.rank_placement.clone());
        self.update_panel(ctx);
        if let Some((file,frame))=self.shot.as_mut(){
            *frame+=1;ctx.request_repaint();
            if *frame==std::env::var("SP_SHOT_FRAME").ok().and_then(|v|v.parse().ok()).unwrap_or(40){ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));}
            let shot=ctx.input(|i|i.events.iter().find_map(|e|if let egui::Event::Screenshot{image,..}=e{Some(image.clone())}else{None}));
            if let Some(image)=shot{
                let pixels:Vec<u8>=image.pixels.iter().flat_map(|c|c.to_array()).collect();
                if let Some(png)=image::RgbaImage::from_raw(image.size[0] as u32,image.size[1] as u32,pixels){let _=png.save(file.as_path());}
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
        if self.smoke.is_some() && self.snapshot["inventory"].is_array() {
            let file = self.smoke.take().unwrap();
            let _=fs::write(file,json!({"gui":"ok","frontend":"Rust","catalog":self.rows.len(),"inventory":self.snapshot["inventory"].as_array().unwrap().len(),"packs":self.packs.len(),"changes":self.state.as_object().map(|v|v.len()),"game_detected":self.game.is_some(),"version":VERSION,"elapsed_ms":self.started.elapsed().as_millis()}).to_string());
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
}
fn main() {
    let root = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();
    let args: Vec<_> = std::env::args().collect();
    if args.get(1).map(String::as_str)==Some("--engine-job-check"){
        if let (Some(input),Some(output))=(args.get(2),args.get(3)){
            let target=PathBuf::from(output);let progress_path=target.with_extension("progress.jsonl");
            let result=worker_progress(&root,&read_json(Path::new(input)),|p|{if let Ok(mut f)=fs::OpenOptions::new().create(true).append(true).open(&progress_path){let _=writeln!(f,"{p}");}});
            let value=match result{Ok(v)=>json!({"ok":true,"result":v}),Err(e)=>json!({"ok":false,"error":e})};
            let _=fs::write(target,value.to_string());
        }
        return;
    }
    if args.get(1).map(String::as_str) == Some("--fps-check") {
        if let (Some(pid), Some(file)) =
            (args.get(2).and_then(|v| v.parse::<u32>().ok()), args.get(3))
        {
            telemetry::check(&root, pid, Path::new(file));
        }
        return;
    }
    if args.get(1).map(String::as_str) == Some("--fps-probe") {
        let _ = fps_probe::run();
        return;
    }
    if args.get(1).map(String::as_str) == Some("--diagnose") {
        let report = json!({"version":VERSION,"game":platform::detect(None).map(|(p,v)|json!({"path":p,"version":v})),"engine":root.join("_engine/SWAPPRISMEngine.exe").is_file(),"updates_published":!s(&platform::config(&root),"update_manifest_url").is_empty()});
        if let Some(p) = args.get(2) {
            let _ = fs::write(p, report.to_string());
        }
        return;
    }
    let smoke = if args.get(1).map(String::as_str) == Some("--smoke") {
        args.get(2).map(PathBuf::from)
    } else {
        None
    };
    let shot=if args.get(1).map(String::as_str)==Some("--screenshot"){args.get(2).cloned().zip(args.get(3).map(PathBuf::from))}else{None};
    let image = image::load_from_memory(include_bytes!("../assets/logo.png"))
        .unwrap()
        .into_rgba8();
    let (w, h) = image.dimensions();
    let name: Vec<u16> = "Local\\SWAPPRISM.SingleInstance\0".encode_utf16().collect();
    let _instance = unsafe {
        windows_sys::Win32::System::Threading::CreateMutexW(std::ptr::null(), 0, name.as_ptr())
    };
    if shot.is_none() && unsafe { windows_sys::Win32::Foundation::GetLastError() } == 183 {
        let title: Vec<u16> = "Swap Prism\0".encode_utf16().collect();
        unsafe {
            let window = windows_sys::Win32::UI::WindowsAndMessaging::FindWindowW(
                std::ptr::null(),
                title.as_ptr(),
            );
            if !window.is_null() {
                windows_sys::Win32::UI::WindowsAndMessaging::ShowWindow(window, 9);
                windows_sys::Win32::UI::WindowsAndMessaging::SetForegroundWindow(window);
            }
        }
        return;
    }
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Swap Prism")
            // All native viewports share this GL configuration. Request alpha
            // here as well, otherwise the FPS child has a black OS background
            // even when its framebuffer pixels are transparent.
            .with_transparent(true)
            .with_inner_size([1280., 860.])
            .with_min_inner_size([1060., 740.])
            .with_icon(egui::IconData {
                rgba: image.into_raw(),
                width: w,
                height: h,
            }),
        ..Default::default()
    };
    if let Err(e) = eframe::run_native(
        "Swap Prism",
        options,
        Box::new(move |cc| {
            let mut app=App::new(cc, root, smoke);
            if let Some((section,file))=shot{
                let(section,tab)=section.split_once(':').unwrap_or((section.as_str(),""));
                app.go_to(section);
                if let Ok(i)=tab.parse(){app.fake_tab=i;}
                if let Some(theme)=std::env::var("SP_THEME").ok(){design::set_theme(&theme);design::setup(&cc.egui_ctx);}
                app.shot=Some((file,0));
            }
            Ok(Box::new(app))
        }),
    ) {
        let _ = rfd::MessageDialog::new()
            .set_title("Swap Prism")
            .set_description(format!("No se pudo abrir la ventana: {e}"))
            .set_level(rfd::MessageLevel::Error)
            .show();
    }
}
