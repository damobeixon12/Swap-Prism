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
    // A full process snapshot is costly; overlays ask several times a second, so remember the game's pid for 3 s.
    static CACHE: std::sync::Mutex<Option<(Option<u32>, std::time::Instant)>> = std::sync::Mutex::new(None);
    let game = {
        let mut cache = CACHE.lock().unwrap();
        match *cache {
            Some((pid, at)) if at.elapsed() < std::time::Duration::from_secs(3) => pid,
            _ => { let pid = process_id("RocketLeague.exe"); *cache = Some((pid, std::time::Instant::now())); pid }
        }
    };
    let Some(game) = game else { return false };
    unsafe {
        let window = windows_sys::Win32::UI::WindowsAndMessaging::GetForegroundWindow();
        let mut pid = 0;
        windows_sys::Win32::UI::WindowsAndMessaging::GetWindowThreadProcessId(window, &mut pid);
        pid == game
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
        // Checked on the window itself: a closed overlay's handle value gets reused by the next one, which then stayed
        // opaque (a black screen) when this was remembered per handle.
        let (mut key,mut alpha,mut flags)=(0u32,0u8,0u32);
        let keyed=GetLayeredWindowAttributes(window,&mut key,&mut alpha,&mut flags)!=0&&flags&LWA_COLORKEY!=0&&key==0;
        if !keyed{SetLayeredWindowAttributes(window,0,255,LWA_COLORKEY);}
    }
}
/// Own top-level window that is never minimized: a web view inside keeps rendering while the app is minimized.
pub fn offscreen_host()->isize{
    use windows_sys::Win32::UI::WindowsAndMessaging::*;
    let class:Vec<u16>="STATIC\0".encode_utf16().collect();let title:Vec<u16>="Swap Prism · Tracker\0".encode_utf16().collect();
    // A visible 1x1 px window behind everything: the full-size page inside is clipped, but Chromium still draws it
    // (it stops for hidden, transparent or off-screen hosts).
    unsafe{
        let window=CreateWindowExW(WS_EX_TOOLWINDOW|WS_EX_NOACTIVATE|WS_EX_TRANSPARENT,class.as_ptr(),title.as_ptr(),WS_POPUP|WS_VISIBLE,0,0,1,1,std::ptr::null_mut(),std::ptr::null_mut(),std::ptr::null_mut(),std::ptr::null());
        if window.is_null(){return 0;}
        SetWindowPos(window,HWND_BOTTOM,0,0,0,0,SWP_NOMOVE|SWP_NOSIZE|SWP_NOACTIVATE);
        window as isize
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
/// True once per press of the scoreboard button: Tab, or View/Back on an XInput pad. Read-only OS queries.
static SCOREBOARD_HELD:std::sync::atomic::AtomicBool=std::sync::atomic::AtomicBool::new(false);
/// XInput buttons for the scoreboard (bit 16/17 = left/right trigger); View/Back until the game's own binding is known.
static SCOREBOARD_PAD:std::sync::atomic::AtomicU32=std::sync::atomic::AtomicU32::new(0x20);
/// Uses the controller button bound to ToggleScoreboard in the game's save (e.g. "XboxTypeS_LeftThumbStick").
pub fn set_scoreboard_keys(keys:&[&str]){
    let mask=keys.iter().map(|k|match k.trim_start_matches("XboxTypeS_"){"A"=>0x1000,"B"=>0x2000,"X"=>0x4000,"Y"=>0x8000,"LeftShoulder"=>0x100,"RightShoulder"=>0x200,
        "LeftThumbStick"=>0x40,"RightThumbStick"=>0x80,"Back"=>0x20,"Start"=>0x10,"DPad_Up"=>0x1,"DPad_Down"=>0x2,"DPad_Left"=>0x4,"DPad_Right"=>0x8,"LeftTrigger"=>1<<16,"RightTrigger"=>1<<17,_=>0}).fold(0u32,|a,b|a|b);
    SCOREBOARD_PAD.store(if mask==0{0x20}else{mask},std::sync::atomic::Ordering::Relaxed);
}
/// Whether the scoreboard button is down right now (as last polled by scoreboard_pressed).
pub fn scoreboard_held()->bool{SCOREBOARD_HELD.load(std::sync::atomic::Ordering::Relaxed)}
pub fn scoreboard_pressed()->bool{
    use std::sync::atomic::Ordering;
    use windows_sys::Win32::UI::Input::{KeyboardAndMouse::{GetAsyncKeyState,VK_TAB},XboxController::{XInputGetState,XINPUT_STATE}};
    use std::sync::atomic::AtomicU32;
    // Polling empty XInput slots is slow and causes stutters in games: only read connected pads, re-probe every ~2 s.
    static CONNECTED:AtomicU32=AtomicU32::new(0);static TICK:AtomicU32=AtomicU32::new(0);
    let probe=TICK.fetch_add(1,Ordering::Relaxed)%64==0;
    let mut down=unsafe{GetAsyncKeyState(VK_TAB as i32)}<0;
    let mut connected=CONNECTED.load(Ordering::Relaxed);
    for pad in 0..4u32{
        if !probe&&connected&(1<<pad)==0{continue;}
        let mut state:XINPUT_STATE=unsafe{std::mem::zeroed()};
        if unsafe{XInputGetState(pad,&mut state)}==0{
            connected|=1<<pad;let g=state.Gamepad;let mask=SCOREBOARD_PAD.load(Ordering::Relaxed);
            if g.wButtons as u32&mask&0xffff!=0||(mask&1<<16!=0&&g.bLeftTrigger>30)||(mask&1<<17!=0&&g.bRightTrigger>30){down=true;}
        }else{connected&=!(1<<pad);}
    }
    CONNECTED.store(connected,Ordering::Relaxed);
    let was=SCOREBOARD_HELD.swap(down,Ordering::Relaxed);
    down&&!was
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

// ---------- tray icon and start with Windows ----------
use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};
use windows_sys::Win32::UI::{Shell::*, WindowsAndMessaging::*};

/// Set by the tray; the F8 thread acts on them (it can reach the window even while it is hidden).
pub static TRAY_TOGGLE: AtomicBool = AtomicBool::new(false);
pub static TRAY_EXIT: AtomicBool = AtomicBool::new(false);
static TRAY_WINDOW: AtomicIsize = AtomicIsize::new(0);
const TRAY_MESSAGE: u32 = WM_APP + 1;

fn wide(text: &str) -> Vec<u16> { text.encode_utf16().chain(Some(0)).collect() }

/// Is the main window («Swap Prism») shown right now.
pub fn main_window_visible() -> bool {
    let title = wide("Swap Prism");
    unsafe { let window = FindWindowW(std::ptr::null(), title.as_ptr()); !window.is_null() && IsWindowVisible(window) != 0 }
}

unsafe extern "system" fn tray_procedure(window: windows_sys::Win32::Foundation::HWND, message: u32, wparam: usize, lparam: isize) -> isize {
    if message == TRAY_MESSAGE {
        match lparam as u32 {
            WM_LBUTTONUP => TRAY_TOGGLE.store(true, Ordering::Relaxed),
            WM_RBUTTONUP => {
                let menu = CreatePopupMenu();
                let (show, exit) = (wide(&crate::design::tr("Mostrar / ocultar  (F8)")), wide(&crate::design::tr("Salir")));
                AppendMenuW(menu, MF_STRING, 1, show.as_ptr());
                AppendMenuW(menu, MF_STRING, 2, exit.as_ptr());
                let mut point = std::mem::zeroed();
                GetCursorPos(&mut point);
                SetForegroundWindow(window); // otherwise the menu does not close when clicking elsewhere
                let choice = TrackPopupMenu(menu, TPM_RETURNCMD | TPM_RIGHTBUTTON, point.x, point.y, 0, window, std::ptr::null());
                PostMessageW(window, WM_NULL, 0, 0);
                DestroyMenu(menu);
                match choice { 1 => TRAY_TOGGLE.store(true, Ordering::Relaxed), 2 => TRAY_EXIT.store(true, Ordering::Relaxed), _ => {} }
            }
            _ => {}
        }
        return 0;
    }
    DefWindowProcW(window, message, wparam, lparam)
}

fn tray_data(window: windows_sys::Win32::Foundation::HWND) -> NOTIFYICONDATAW {
    let mut data: NOTIFYICONDATAW = unsafe { std::mem::zeroed() };
    data.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
    data.hWnd = window;
    data.uID = 1;
    data
}

/// Tray icon on its own thread (hidden window + message loop). `rgba` is a 32×32 picture.
pub fn tray(rgba: Vec<u8>) {
    std::thread::spawn(move || unsafe {
        let class = wide("SwapPrismTray");
        let mut kind: WNDCLASSW = std::mem::zeroed();
        kind.lpfnWndProc = Some(tray_procedure);
        kind.lpszClassName = class.as_ptr();
        RegisterClassW(&kind);
        let title = wide("Swap Prism tray");
        let window = CreateWindowExW(0, class.as_ptr(), title.as_ptr(), 0, 0, 0, 0, 0, std::ptr::null_mut(), std::ptr::null_mut(), std::ptr::null_mut(), std::ptr::null());
        if window.is_null() { return; }
        let bgra: Vec<u8> = rgba.chunks(4).flat_map(|p| [p[2], p[1], p[0], p[3]]).collect();
        let mask = [0u8; 32 * 32 / 8];
        let mut data = tray_data(window);
        data.uFlags = NIF_ICON | NIF_MESSAGE | NIF_TIP;
        data.uCallbackMessage = TRAY_MESSAGE;
        data.hIcon = CreateIcon(std::ptr::null_mut(), 32, 32, 1, 32, mask.as_ptr(), bgra.as_ptr());
        for (slot, unit) in data.szTip.iter_mut().zip(wide(&crate::design::tr("Swap Prism · F8 para mostrar u ocultar"))) { *slot = unit; }
        if Shell_NotifyIconW(NIM_ADD, &data) == 0 { return; }
        TRAY_WINDOW.store(window as isize, Ordering::Relaxed);
        let mut message: MSG = std::mem::zeroed();
        while GetMessageW(&mut message, std::ptr::null_mut(), 0, 0) > 0 { DispatchMessageW(&message); }
    });
}

/// Takes the icon away on exit (otherwise it lingers until the mouse passes over it).
pub fn tray_remove() {
    let window = TRAY_WINDOW.swap(0, Ordering::Relaxed);
    if window != 0 { unsafe { Shell_NotifyIconW(NIM_DELETE, &tray_data(window as _)); } }
}

const RUN_KEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";
const RUN_VALUE: &str = "Swap Prism";

/// Starts with Windows (current user only), hidden in the tray.
pub fn autostart() -> bool {
    use windows_sys::Win32::System::Registry::*;
    let (key, value) = (wide(RUN_KEY), wide(RUN_VALUE));
    let mut size = 0u32;
    unsafe { RegGetValueW(HKEY_CURRENT_USER, key.as_ptr(), value.as_ptr(), RRF_RT_REG_SZ, std::ptr::null_mut(), std::ptr::null_mut(), &mut size) == 0 }
}

pub fn set_autostart(on: bool) -> Result<(), String> {
    use windows_sys::Win32::System::Registry::*;
    let (key, value) = (wide(RUN_KEY), wide(RUN_VALUE));
    let code = unsafe {
        if on {
            let exe = std::env::current_exe().map_err(|e| e.to_string())?;
            let command = wide(&format!("\"{}\" --tray", exe.display()));
            RegSetKeyValueW(HKEY_CURRENT_USER, key.as_ptr(), value.as_ptr(), REG_SZ, command.as_ptr().cast(), (command.len() * 2) as u32)
        } else {
            RegDeleteKeyValueW(HKEY_CURRENT_USER, key.as_ptr(), value.as_ptr())
        }
    };
    if code == 0 || (!on && code == 2) { Ok(()) } else { Err(format!("No se pudo cambiar el inicio con Windows (código {code}).")) }
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
