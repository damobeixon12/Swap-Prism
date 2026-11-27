#![cfg_attr(not(test), windows_subsystem = "windows")]
use std::{fs::{self,File},io::{Read,Write},path::{Path,PathBuf},process::Command,sync::{Arc,Mutex},time::Duration};
use eframe::egui;
use sha2::{Digest,Sha256};
use windows_sys::Win32::{Foundation::{CloseHandle,GetLastError},System::Threading::{OpenProcess,WaitForSingleObject},UI::WindowsAndMessaging::{MessageBoxW,MB_ICONERROR,MB_OK}};
#[allow(dead_code)]#[path="design.rs"]mod design;
#[derive(Clone)]struct Config{pid:u32,installer:PathBuf,root:PathBuf,hash:String}
#[derive(Clone)]struct Status{fraction:f32,label:String,error:String,done:bool}
fn verify(installer:&Path,root:&Path,expected:&str)->Result<(),String>{
 if expected.len()!=64||!expected.bytes().all(|b|b.is_ascii_hexdigit()){return Err("SHA-256 no válido.".into());}
 if fs::canonicalize(installer).map_err(|e|e.to_string())?!=fs::canonicalize(root.join("data/updates/Swap Prism-Setup.exe")).map_err(|e|e.to_string())?{return Err("Ruta del instalador no válida.".into());}
 let mut file=File::open(installer).map_err(|e|e.to_string())?;let mut digest=Sha256::new();let mut buf=[0;65536];
 loop{let n=file.read(&mut buf).map_err(|e|e.to_string())?;if n==0{break;}digest.update(&buf[..n]);}
 if format!("{:x}",digest.finalize())!=expected.to_ascii_lowercase(){return Err("El instalador cambió después de verificar la descarga.".into());}Ok(())
}
fn wait_for_owner(pid:u32)->Result<(),String>{unsafe{let process=OpenProcess(0x00100000,0,pid);if process.is_null(){let error=GetLastError();return if error==87{Ok(())}else{Err(format!("No se pudo esperar al proceso: {error}"))};}let result=WaitForSingleObject(process,60000);CloseHandle(process);if result==0{Ok(())}else{Err("Swap Prism no terminó de cerrarse.".into())}}}
fn stage(state:&Arc<Mutex<Status>>,ctx:&egui::Context,fraction:f32,label:&str){let mut s=state.lock().unwrap();s.fraction=s.fraction.max(fraction);s.label=label.into();ctx.request_repaint();}
fn install(config:&Config,state:&Arc<Mutex<Status>>,ctx:&egui::Context)->Result<(),String>{
 let folder=config.root.join("data/updates");let progress=folder.join("install-progress.txt");let _=fs::remove_file(&progress);
 let mut log=File::create(folder.join("update-runner.log")).map_err(|e|e.to_string())?;writeln!(log,"Native updater waiting for owner {}",config.pid).ok();
 wait_for_owner(config.pid)?;stage(state,ctx,0.03,"Comprobando el instalador…");verify(&config.installer,&config.root,&config.hash)?;
 writeln!(log,"Owner closed; SHA-256 verified; starting silent installer.").ok();
 let mut child=Command::new(&config.installer).arg("/S").arg(format!("/D={}",config.root.display())).spawn().map_err(|e|format!("No se pudo abrir el instalador: {e}"))?;
 loop{if let Some(exit)=child.try_wait().map_err(|e|e.to_string())?{if !exit.success(){return Err(format!("El instalador devolvió {}",exit.code().unwrap_or(-1)));}break;}let amount=fs::read_to_string(&progress).ok().and_then(|s|s.trim().parse::<u32>().ok()).unwrap_or(0).min(100);stage(state,ctx,0.08+amount as f32*0.0087,"Instalando la actualización…");std::thread::sleep(Duration::from_millis(80));}
 stage(state,ctx,1.,"Actualización completada. Reiniciando…");writeln!(log,"Installer completed successfully; reopening application.").ok();std::thread::sleep(Duration::from_millis(450));
 Command::new(config.root.join("Swap Prism.exe")).spawn().map_err(|e|format!("La versión se instaló, pero no pudo reabrirse: {e}"))?;state.lock().unwrap().done=true;ctx.request_repaint();Ok(())
}
struct Updater{config:Config,state:Arc<Mutex<Status>>,frames:u32,allow_close:bool}
impl eframe::App for Updater{
 fn update(&mut self,ctx:&egui::Context,_:&mut eframe::Frame){
  self.frames+=1;if self.frames==3{let _=fs::write(self.config.root.join("data/updates/runner-ready.json"),format!("{{\"owner_pid\":{}}}",self.config.pid));}
  let status=self.state.lock().unwrap().clone();if ctx.input(|i|i.viewport().close_requested())&&!self.allow_close{ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);}
  egui::CentralPanel::default().frame(egui::Frame::new().fill(design::BACKGROUND()).inner_margin(28)).show(ctx,|ui|{ui.vertical_centered(|ui|{
   ui.add_space(10.);ui.add(egui::Image::new(egui::include_image!("../assets/logo.png")).fit_to_exact_size(egui::Vec2::splat(90.)));ui.add_space(12.);ui.heading("Swap Prism");ui.add_space(12.);ui.label(&status.label);ui.add_space(12.);ui.add(egui::ProgressBar::new(status.fraction).desired_width(440.).show_percentage());ui.add_space(12.);
   if status.error.is_empty(){ui.label("Tus selecciones y copias se conservan.");}else{ui.label(egui::RichText::new(&status.error).color(design::RED()));if ui.button("Reabrir Swap Prism").clicked(){let _=Command::new(self.config.root.join("Swap Prism.exe")).spawn();self.allow_close=true;ctx.send_viewport_cmd(egui::ViewportCommand::Close);}if ui.button("Cerrar").clicked(){self.allow_close=true;ctx.send_viewport_cmd(egui::ViewportCommand::Close);}}
  });});if status.done{self.allow_close=true;ctx.send_viewport_cmd(egui::ViewportCommand::Close);}ctx.request_repaint_after(Duration::from_millis(100));
 }
}
fn run()->Result<(),String>{
 let args:Vec<_>=std::env::args_os().collect();if args.len()!=6||args[1]!="--install"{return Err("Este componente se abre desde Actualizaciones de Swap Prism.".into());}
 let config=Config{pid:args[2].to_str().and_then(|s|s.parse::<u32>().ok()).filter(|n|*n>0).ok_or("Proceso no válido.")?,installer:PathBuf::from(&args[3]),root:PathBuf::from(&args[4]),hash:args[5].to_str().ok_or("SHA-256 no válido.")?.into()};
 let logo=image::load_from_memory(include_bytes!("../assets/logo.png")).map_err(|e|e.to_string())?.into_rgba8();let(width,height)=logo.dimensions();let options=eframe::NativeOptions{viewport:egui::ViewportBuilder::default().with_title("Swap Prism · Actualizando").with_inner_size([540.,410.]).with_resizable(false).with_icon(egui::IconData{rgba:logo.into_raw(),width,height}),..Default::default()};
 eframe::run_native("Swap Prism · Actualizando",options,Box::new(move|cc|{design::setup(&cc.egui_ctx);egui_extras::install_image_loaders(&cc.egui_ctx);let state=Arc::new(Mutex::new(Status{fraction:0.,label:"Preparando la actualización…".into(),error:String::new(),done:false}));let copy=state.clone();let context=cc.egui_ctx.clone();let task=config.clone();std::thread::spawn(move||{if let Err(error)=install(&task,&copy,&context){let _=fs::write(task.root.join("data/updates/update-error.txt"),&error);let mut s=copy.lock().unwrap();s.label="No se pudo completar la actualización".into();s.error=error;context.request_repaint();}});Ok(Box::new(Updater{config,state,frames:0,allow_close:false}))})).map_err(|e|e.to_string())
}
fn main(){if let Err(error)=run(){let text:Vec<u16>=format!("No se pudo abrir el actualizador.\n\n{error}").encode_utf16().chain(Some(0)).collect();let title:Vec<u16>="Swap Prism · Actualización".encode_utf16().chain(Some(0)).collect();unsafe{MessageBoxW(std::ptr::null_mut(),text.as_ptr(),title.as_ptr(),MB_OK|MB_ICONERROR);}}}
#[cfg(test)]mod tests{use super::*;
 #[test]fn rejects_wrong_hash_and_outside_path(){let root=std::env::temp_dir().join(format!("swapprism-runner-{}",std::process::id()));fs::create_dir_all(root.join("data/updates")).unwrap();let path=root.join("data/updates/Swap Prism-Setup.exe");fs::write(&path,b"installer").unwrap();assert!(verify(&path,&root,&"0".repeat(64)).is_err());let hash=format!("{:x}",Sha256::digest(b"installer"));assert!(verify(&path,&root,&hash).is_ok());let other=root.join("other.exe");fs::write(&other,b"installer").unwrap();assert!(verify(&other,&root,&hash).is_err());fs::remove_dir_all(root).unwrap();}
}
