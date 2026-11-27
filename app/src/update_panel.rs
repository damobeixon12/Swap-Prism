use super::*;
use crate::design as d;
impl App{
 pub(super) fn check_updates_ui(&mut self){
  self.update_open=true;self.update_checking=true;self.update_error.clear();
  let url=s(&self.config,"update_manifest_url").to_owned();let tx=self.tx.clone();
  thread::spawn(move||{let _=tx.send(Event::Update(update::check(&url,VERSION)));});
 }
 pub(super) fn start_native_update(&mut self){
  if self.running||self.busy{return;}
  if let Some(path)=self.update_file.clone(){match update::install_after_exit(&path,&self.root,&self.update_sha256){
   Ok(())=>{self.update_waiting=Some(Instant::now());self.update_error.clear();},Err(e)=>self.update_error=e
  }}
 }
 /// Silent re-check every 30 min; downloads as soon as the app is idle and installs once Rocket League is closed.
 pub(super) fn auto_update(&mut self,ctx:&egui::Context){
  ctx.request_repaint_after(Duration::from_secs(60));
  let url=s(&self.config,"update_manifest_url").to_owned();
  if !url.is_empty()&&!self.update_checking&&self.available_update.is_none()&&self.update_file.is_none()&&self.update_checked_at.elapsed()>Duration::from_secs(1800){
   self.update_checked_at=Instant::now();let tx=self.tx.clone();
   thread::spawn(move||{let _=tx.send(Event::Update(update::check(&url,VERSION)));});
  }
  if self.update_auto&&!self.busy&&!self.update_downloading{
   if let Some(release)=self.available_update.clone(){self.update_auto=false;self.download_update(release);}
  }
  if self.update_file.is_some()&&self.update_waiting.is_none()&&!self.busy&&!self.running&&self.update_error.is_empty(){self.start_native_update();}
 }
 fn download_update(&mut self,release:update::Release){
  self.busy=true;self.update_downloading=true;self.update_bytes=0;self.update_total=None;self.update_error.clear();
  let tx=self.tx.clone();let root=self.root.clone();thread::spawn(move||{let result=update::download_progress(&release,&root,|bytes,total|{let _=tx.send(Event::UpdateBytes(bytes,total));});let _=tx.send(Event::Downloaded(result));});
 }
 pub(super) fn update_panel(&mut self,ctx:&egui::Context){
  if !self.update_open{return;}
  if let Some(started)=self.update_waiting{
   let ready=read_json(&self.root.join("data/updates/runner-ready.json"));
   if ready["owner_pid"].as_u64()==Some(std::process::id()as u64){ctx.send_viewport_cmd(egui::ViewportCommand::Close);}
   else if started.elapsed().as_secs()>25{self.update_waiting=None;self.update_error=crate::i18n::t("El lanzador no respondió. La app sigue abierta; vuelve a intentarlo.").into();}
   ctx.request_repaint_after(Duration::from_millis(100));
  }
  egui::Window::new(crate::i18n::t("Actualizar Swap Prism")).collapsible(false).resizable(false)
   .anchor(egui::Align2::CENTER_CENTER,Vec2::ZERO).default_width(480.).show(ctx,|ui|{
    ui.vertical_centered(|ui|{
     ui.add_space(14.);ui.add(egui::Image::new(egui::include_image!("../assets/logo.png")).fit_to_exact_size(Vec2::splat(72.)));
     ui.add_space(10.);ui.heading("Swap Prism");
     if self.update_checking{ui.spinner();ui.label(crate::i18n::t("Buscando actualizaciones…"));}
     else if self.update_waiting.is_some(){ui.spinner();ui.label(crate::i18n::t("Preparando la instalación…"));d::muted(ui,crate::i18n::t("Se abrirá el progreso y la app se reiniciará al terminar."));}
     else if self.update_downloading{
      ui.label(crate::i18n::t("Descargando actualización…"));
      let ratio=self.update_total.map(|n|self.update_bytes as f32/n.max(1)as f32).unwrap_or(0.);
      ui.add(egui::ProgressBar::new(ratio.clamp(0.,1.)).show_percentage().desired_width(410.));
      ui.label(match self.update_total{Some(total)=>format!("{:.1} / {:.1} MB",self.update_bytes as f64/1048576.,total as f64/1048576.),None=>crate::i18n::tr(&format!("{:.1} MB recibidos",self.update_bytes as f64/1048576.))});
     }else if let Some(release)=self.available_update.clone(){
      ui.label(RichText::new(crate::i18n::tr(&format!("Versión {} disponible",release.version))).size(19.));
      if let Some(notes)=&release.notes{d::muted(ui,notes);}
      ui.add_space(10.);if ui.add_enabled(!self.busy,d::button(crate::i18n::t("Actualizar ahora"),d::GREEN())).clicked(){self.update_auto=false;self.download_update(release);}
      if self.busy{d::muted(ui,crate::i18n::t("Se descargará sola en cuanto termine lo que está haciendo la app."));}
      if self.running{d::muted(ui,crate::i18n::t("Cierra Rocket League para instalar la actualización."));}
      d::muted(ui,crate::i18n::t("Descarga verificada, instalación y reinicio automático."));
     }else if self.update_file.is_some(){ui.label(crate::i18n::t("Descarga verificada"));if self.running{d::muted(ui,crate::i18n::t("Se instalará sola al cerrar Rocket League."));}if ui.add_enabled(!self.busy&&!self.running,d::button(crate::i18n::t("Instalar y reiniciar"),d::GREEN())).clicked(){self.start_native_update();}}
     else{ui.label(crate::i18n::t("Ya tienes la última versión publicada."));}
     if !self.update_error.is_empty(){ui.label(RichText::new(&self.update_error).color(d::RED()));}
     ui.add_space(12.);if !self.update_downloading&&self.update_waiting.is_none()&&ui.button(crate::i18n::t("Cerrar")).clicked(){self.update_open=false;}
     ui.add_space(12.);
    });
   });
 }
}
