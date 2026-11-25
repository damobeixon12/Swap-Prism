use serde_json::{json, Value};
use std::{
    fs,
    path::{Path, PathBuf},
};
use windows_sys::Win32::{
    Foundation::{CloseHandle, INVALID_HANDLE_VALUE},
    System::Diagnostics::ToolHelp::*,
};

pub fn running() -> bool {
    unsafe {
        let h = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if h == INVALID_HANDLE_VALUE {
            return true;
        }
        let mut entry: PROCESSENTRY32W = std::mem::zeroed();
        entry.dwSize = std::mem::size_of_val(&entry) as u32;
        let mut ok = Process32FirstW(h, &mut entry);
        let mut found = false;
        while ok != 0 {
            let n = entry.szExeFile.iter().position(|c| *c == 0).unwrap_or(260);
            if String::from_utf16_lossy(&entry.szExeFile[..n])
                .eq_ignore_ascii_case("RocketLeague.exe")
            {
                found = true;
                break;
            }
            ok = Process32NextW(h, &mut entry);
        }
        CloseHandle(h);
        found
    }
}

pub fn process_id(name: &str) -> Option<u32> {
    unsafe {
        let h = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if h == INVALID_HANDLE_VALUE {
            return None;
        }
        let mut entry: PROCESSENTRY32W = std::mem::zeroed();
        entry.dwSize = std::mem::size_of_val(&entry) as u32;
        let mut ok = Process32FirstW(h, &mut entry);
        let mut found = None;
        while ok != 0 {
            let n = entry.szExeFile.iter().position(|c| *c == 0).unwrap_or(260);
            if String::from_utf16_lossy(&entry.szExeFile[..n]).eq_ignore_ascii_case(name) {
                found = Some(entry.th32ProcessID);
                break;
            }
            ok = Process32NextW(h, &mut entry);
        }
        CloseHandle(h);
        found
    }
}
pub fn foreground_game() -> bool {
    unsafe {
        let window = windows_sys::Win32::UI::WindowsAndMessaging::GetForegroundWindow();
        let mut pid = 0;
        windows_sys::Win32::UI::WindowsAndMessaging::GetWindowThreadProcessId(window, &mut pid);
        process_id("RocketLeague.exe") == Some(pid)
    }
}

/// Some Windows/OpenGL drivers present transparent child surfaces as opaque
/// black. A color key on our own borderless window removes that fallback matte.
/// Never touches the game window or another process's overlay.
pub fn transparent_overlay(title:&str,editing:bool) {
    use windows_sys::Win32::UI::WindowsAndMessaging::*;
    unsafe {
        let title:Vec<u16>=title.encode_utf16().chain([0]).collect();
        let window=FindWindowW(std::ptr::null(),title.as_ptr());
        if window.is_null(){return;}
        let mut pid=0;GetWindowThreadProcessId(window,&mut pid);
        if pid!=std::process::id(){return;}
        let style=GetWindowLongPtrW(window,GWL_EXSTYLE);
        let mut desired=(style|WS_EX_LAYERED as isize|WS_EX_TOOLWINDOW as isize)&!(WS_EX_APPWINDOW as isize);
        if editing{desired&=!(WS_EX_NOACTIVATE as isize);}else{desired|=WS_EX_NOACTIVATE as isize;}
        if style!=desired{SetWindowLongPtrW(window,GWL_EXSTYLE,desired);}
        static CONFIGURED:std::sync::Mutex<Vec<isize>>=std::sync::Mutex::new(Vec::new());
        let mut done=CONFIGURED.lock().unwrap();
        if !done.contains(&(window as isize)){done.push(window as isize);SetLayeredWindowAttributes(window,0,255,LWA_COLORKEY);}
    }
}
pub fn overlay_visibility(title:&str,visible:bool){
    use windows_sys::Win32::UI::WindowsAndMessaging::*;
    unsafe{
        let title:Vec<u16>=title.encode_utf16().chain([0]).collect();let window=FindWindowW(std::ptr::null(),title.as_ptr());if window.is_null(){return;}
        let mut pid=0;GetWindowThreadProcessId(window,&mut pid);if pid!=std::process::id(){return;}
        if (IsWindowVisible(window)!=0)!=visible{ShowWindow(window,if visible{SW_SHOWNOACTIVATE}else{SW_HIDE});}
    }
}
pub fn game_bounds() -> Option<[f32; 4]> {
    struct Search {
        pid: u32,
        rect: Option<[f32; 4]>,
    }
    unsafe extern "system" fn find(
        window: windows_sys::Win32::Foundation::HWND,
        data: isize,
    ) -> i32 {
        let search = &mut *(data as *mut Search);
        let mut pid = 0;
        windows_sys::Win32::UI::WindowsAndMessaging::GetWindowThreadProcessId(window, &mut pid);
        if pid == search.pid
            && windows_sys::Win32::UI::WindowsAndMessaging::IsWindowVisible(window) != 0
        {
            let mut rect = std::mem::zeroed::<windows_sys::Win32::Foundation::RECT>();
            if windows_sys::Win32::UI::WindowsAndMessaging::GetWindowRect(window, &mut rect) != 0
                && rect.right - rect.left > 100
            {
                search.rect = Some([
                    rect.left as f32,
                    rect.top as f32,
                    rect.right as f32,
                    rect.bottom as f32,
                ]);
                return 0;
            }
        }
        1
    }
    let mut search = Search {
        pid: process_id("RocketLeague.exe")?,
        rect: None,
    };
    unsafe {
        windows_sys::Win32::UI::WindowsAndMessaging::EnumWindows(
            Some(find),
            &mut search as *mut _ as isize,
        );
    }
    search.rect
}
pub fn valid_game(path: &Path) -> bool {
    path.join("TAGame/CookedPCConsole").is_dir()
        && path.join("Binaries/Win64/RocketLeague.exe").is_file()
}
pub fn detect(saved: Option<&str>) -> Option<(PathBuf, String)> {
    if let Some(s) = saved {
        let p = PathBuf::from(s);
        if valid_game(&p) {
            return Some((p, String::new()));
        }
    }
    let base = std::env::var_os("PROGRAMDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("C:/ProgramData"));
    if let Ok(files) = fs::read_dir(base.join("Epic/EpicGamesLauncher/Data/Manifests")) {
        for f in files.flatten() {
            if f.path().extension().and_then(|s| s.to_str()) != Some("item") {
                continue;
            }
            if let Ok(raw) = fs::read_to_string(f.path()) {
                if let Ok(v) = serde_json::from_str::<Value>(raw.trim_start_matches('\u{feff}')) {
                    if v["DisplayName"]
                        .as_str()
                        .unwrap_or("")
                        .to_lowercase()
                        .contains("rocket league")
                    {
                        if let Some(s) = v["InstallLocation"].as_str() {
                            let p = PathBuf::from(s);
                            if valid_game(&p) {
                                return Some((
                                    p,
                                    v["AppVersionString"].as_str().unwrap_or("").into(),
                                ));
                            }
                        }
                    }
                }
            }
        }
    }
    for drive in b'C'..=b'Z' {
        for suffix in [
            "Program Files/Epic Games/rocketleague",
            "Epic Games/rocketleague",
            "EpicGames/rocketleague",
            "Games/rocketleague",
            "Program Files (x86)/Steam/steamapps/common/rocketleague",
            "SteamLibrary/steamapps/common/rocketleague",
        ] {
            let p = PathBuf::from(format!("{}:/{suffix}", drive as char));
            if valid_game(&p) {
                return Some((p, String::new()));
            }
        }
    }
    None
}
pub fn config(root: &Path) -> Value {
    let mut defaults:Value=serde_json::from_slice(include_bytes!("../assets/settings.json")).unwrap_or(json!({}));
    let saved:Value=fs::read_to_string(root.join("data/settings.json"))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or(json!({}));
    if let (Some(dst),Some(src))=(defaults.as_object_mut(),saved.as_object()){for(k,v)in src{if k=="update_manifest_url"&&v.as_str()==Some(""){continue;}dst.insert(k.clone(),v.clone());}}
    defaults
}
pub fn thumbnail_path(root:&Path,url:&str)->PathBuf{
    use sha2::{Digest,Sha256};
    let mut path=root.join(format!("data/thumbnails/{:x}.webp",Sha256::digest(url.as_bytes())));
    if !path.is_file(){path.set_extension("png");}
    path
}
pub fn save_config(root: &Path, v: &Value) -> Result<(), String> {
    fs::create_dir_all(root.join("data")).map_err(|e| e.to_string())?;
    let path = root.join("data/settings.json.tmp");
    fs::write(&path, serde_json::to_vec_pretty(v).unwrap()).map_err(|e| e.to_string())?;
    fs::rename(path, root.join("data/settings.json")).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_arbitrary_directory() {
        assert!(!valid_game(Path::new("C:/Windows")));
    }
    #[test]
    fn old_empty_channel_uses_shipped_default(){
        let root=std::env::temp_dir().join(format!("swapprism-config-check-{}",std::process::id()));
        fs::create_dir_all(root.join("data")).unwrap();
        fs::write(root.join("data/settings.json"),r#"{"update_manifest_url":"","custom":"kept"}"#).unwrap();
        let saved=config(&root);assert_eq!(saved["custom"],"kept");assert!(saved["update_manifest_url"].as_str().unwrap().contains("damobeixon12/Swap-Prism"));
        fs::remove_dir_all(root).unwrap();
    }
}
