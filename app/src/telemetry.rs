//! Read-only frame presentation telemetry from Intel's PresentMon console.
//! No injected DLL, driver installation, process-memory reads, or game commands.
use super::*;
use std::{
    collections::VecDeque,
    io::{BufRead, BufReader},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};
pub const PRESENTMON_SHA256: &str =
    "b2a706bc6ad475749e3b7e3409263aa1e6906d45bdcf993f6dbc0f660188f1af";

struct CollectorJob(windows_sys::Win32::Foundation::HANDLE);
impl CollectorJob {
    fn attach(pid:u32)->Option<Self>{
        use windows_sys::Win32::{Foundation::CloseHandle,System::{JobObjects::*,Threading::*}};
        unsafe {
            let job=CreateJobObjectW(std::ptr::null(),std::ptr::null());if job.is_null(){return None;}
            let mut limits:JOBOBJECT_EXTENDED_LIMIT_INFORMATION=std::mem::zeroed();limits.BasicLimitInformation.LimitFlags=JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            let process=OpenProcess(PROCESS_SET_QUOTA|PROCESS_TERMINATE,0,pid);
            let ok=SetInformationJobObject(job,JobObjectExtendedLimitInformation,&limits as *const _ as *const _,std::mem::size_of_val(&limits)as u32)!=0&&!process.is_null()&&AssignProcessToJobObject(job,process)!=0;
            if !process.is_null(){CloseHandle(process);}if ok{Some(Self(job))}else{CloseHandle(job);None}
        }
    }
}
impl Drop for CollectorJob{fn drop(&mut self){unsafe{windows_sys::Win32::Foundation::CloseHandle(self.0);}}}

#[derive(Clone, Default)]
pub struct Reading {
    pub fps: Option<u32>,
    pub updated: Option<Instant>,
    pub pid: Option<u32>,
    pub status: String,
    pub generation: u64,
}
impl Reading {
    pub fn live_fps(&self) -> Option<u32> {
        if self
            .updated
            .map(|t| t.elapsed() < Duration::from_millis(1500))
            .unwrap_or(false)
        {
            self.fps
        } else {
            None
        }
    }
}
pub struct Controller {
    pub reading: Arc<Mutex<Reading>>,
    enabled: Arc<AtomicBool>,
    stop: Arc<AtomicBool>,
    handle: Option<thread::JoinHandle<()>>,
}
impl Controller {
    pub fn new(root: PathBuf, ctx: egui::Context, enabled: bool) -> Self {
        let reading = Arc::new(Mutex::new(Reading::default()));
        let active = Arc::new(AtomicBool::new(enabled));
        let stop = Arc::new(AtomicBool::new(false));
        let shared = reading.clone();
        let switch = active.clone();
        let quit = stop.clone();
        let handle = thread::spawn(move || supervise(root, shared, switch, quit, ctx));
        Self {
            reading,
            enabled: active,
            stop,
            handle: Some(handle),
        }
    }
    pub fn enable(&self, value: bool) {
        self.enabled.store(value, Ordering::Relaxed);
        if !value {
            let mut r = self.reading.lock().unwrap();
            r.fps = None;
            r.updated = None;
            r.status = "Desactivado".into();
        }
    }
    pub fn snapshot(&self) -> Reading {
        self.reading.lock().unwrap().clone()
    }
}
impl Drop for Controller {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

pub fn verify_tool(path: &Path) -> Result<(), String> {
    let bytes = fs::read(path).map_err(|e| format!("No se encuentra el medidor de FPS: {e}"))?;
    if format!("{:x}", Sha256::digest(bytes)) != PRESENTMON_SHA256 {
        return Err(
            "El medidor de FPS no coincide con su versión verificada. Reinstala SWAPPRISM.".into(),
        );
    }
    Ok(())
}
pub fn command(path: &Path, pid: u32) -> Command {
    let mut cmd = Command::new(path);
    cmd.args([
        "--process_id",
        &pid.to_string(),
        "--output_stdout",
        "--v1_metrics",
        "--no_console_stats",
        "--no_track_input",
        "--session_name",
        &format!("SWAPPRISM-FPS-{}-{pid}", std::process::id()),
    ])
    .creation_flags(0x08000000)
    .stdout(Stdio::piped())
    .stderr(Stdio::null());
    cmd
}

/// ETW sessions outlive a PresentMon that was killed (e.g. Swap Prism closed abruptly). Leftovers from
/// earlier runs keep the frame events and leave the new session with nothing, so stop them first.
fn stop_stale_sessions() {
    let own = format!("SWAPPRISM-FPS-{}-", std::process::id());
    let Ok(out) = Command::new("logman").args(["query", "-ets"]).creation_flags(0x08000000).output() else { return };
    for name in String::from_utf8_lossy(&out.stdout).lines().filter_map(|l| l.split_whitespace().next()) {
        if name.starts_with("SWAPPRISM-FPS-") && !name.starts_with(&own) {
            let _ = Command::new("logman").args(["stop", name, "-ets"]).creation_flags(0x08000000).stdout(Stdio::null()).stderr(Stdio::null()).status();
        }
    }
}

fn stop_session(path: &Path, pid: u32) {
    if verify_tool(path).is_ok() {
        let child = Command::new(path)
            .args([
                "--terminate_existing_session",
                "--session_name",
                &format!("SWAPPRISM-FPS-{}-{pid}", std::process::id()),
            ])
            .creation_flags(0x08000000)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();
        if let Ok(mut child)=child {
            let deadline=Instant::now()+Duration::from_secs(2);
            while child.try_wait().ok().flatten().is_none()&&Instant::now()<deadline{thread::sleep(Duration::from_millis(50));}
            let _=child.kill();let _=child.wait();
        }
    }
}

pub fn check(root: &Path, pid: u32, file: &Path) {
    let tool = root.join("_telemetry/PresentMon.exe");
    let report = (|| -> Result<Value, String> {
        verify_tool(&tool)?;
        let output = command(&tool, pid)
            .args(["--timed", "5", "--terminate_after_timed"])
            .output()
            .map_err(|e| e.to_string())?;
        let mut parser = FrameParser::new(pid);
        let mut values = Vec::new();
        let text = String::from_utf8_lossy(&output.stdout);
        for line in text.lines() {
            if let Some(fps) = parser.feed(line) {
                values.push(fps);
            }
        }
        Ok(
            json!({"source":"PresentMon 2.6.0","target_pid":pid,"exit_code":output.status.code(),"samples":values.len(),"last_fps":values.last(),"csv_lines":text.lines().count(),"verified_tool_sha256":PRESENTMON_SHA256,"game_memory_reads":false}),
        )
    })();
    let value = report.unwrap_or_else(|error| json!({"error":error}));
    let _ = fs::write(file, value.to_string());
}

fn supervise(
    root: PathBuf,
    shared: Arc<Mutex<Reading>>,
    enabled: Arc<AtomicBool>,
    stop: Arc<AtomicBool>,
    ctx: egui::Context,
) {
    let tool = root.join("_telemetry/PresentMon.exe");
    let mut generation = 0u64;
    while !stop.load(Ordering::Relaxed) {
        if !enabled.load(Ordering::Relaxed) {
            thread::sleep(Duration::from_millis(200));
            continue;
        }
        let Some(pid) = platform::process_id("RocketLeague.exe") else {
            let mut r = shared.lock().unwrap();
            r.fps = None;
            r.updated = None;
            r.pid = None;
            r.status = "Esperando a Rocket League".into();
            drop(r);
            thread::sleep(Duration::from_millis(500));
            continue;
        };
        if let Err(error) = verify_tool(&tool) {
            shared.lock().unwrap().status = error;
            thread::sleep(Duration::from_secs(2));
            continue;
        }
        generation += 1;
        {
            let mut r = shared.lock().unwrap();
            *r = Reading {
                pid: Some(pid),
                generation,
                status: "Esperando fotogramas del juego".into(),
                ..Default::default()
            };
        }
        stop_stale_sessions();
        let mut child = match command(&tool, pid).spawn() {
            Ok(child) => child,
            Err(error) => {
                shared.lock().unwrap().status = format!("No se pudo iniciar el medidor: {error}");
                thread::sleep(Duration::from_secs(2));
                continue;
            }
        };
        let _collector_job=CollectorJob::attach(child.id());
        let output = child.stdout.take().unwrap();
        let data = shared.clone();
        let paint = ctx.clone();
        let current = generation;
        let reader = thread::spawn(move || {
            let mut parser = FrameParser::new(pid);
            let mut last_publish = Instant::now() - Duration::from_secs(1);
            for line in BufReader::new(output).lines() {
                let Ok(line) = line else { break };
                if let Some(fps) = parser.feed(&line) {
                    if last_publish.elapsed() >= Duration::from_millis(200) {
                        let mut r = data.lock().unwrap();
                        if r.generation != current {
                            break;
                        }
                        r.fps = Some(fps);
                        r.updated = Some(Instant::now());
                        r.status = "FPS del juego · PresentMon".into();
                        drop(r);
                        paint.request_repaint();
                        paint.request_repaint_of(egui::ViewportId::from_hash_of("swapprism-metrics"));
                        last_publish = Instant::now();
                    }
                }
            }
        });
        let mut failed = None;
        let began=Instant::now();
        while !stop.load(Ordering::Relaxed)
            && enabled.load(Ordering::Relaxed)
            && platform::process_id("RocketLeague.exe") == Some(pid)
        {
            let last=shared.lock().unwrap().updated;
            if platform::foreground_game()&&began.elapsed()>Duration::from_secs(8)&&last.map(|v|v.elapsed()>Duration::from_secs(5)).unwrap_or(true){failed=Some(-2);break;}
            match child.try_wait() {
                Ok(Some(exit)) => {
                    failed = Some(exit.code().unwrap_or(-1));
                    break;
                }
                Ok(None) => {}
                Err(_) => break,
            }
            thread::sleep(Duration::from_millis(200));
        }
        {
            let mut r = shared.lock().unwrap();
            r.generation += 1;
            r.fps = None;
            r.updated = None;
            r.status = if let Some(code) = failed {
                format!("El medidor se detuvo (código {code}).")
            } else {
                "Esperando a Rocket League".into()
            };
        }
        stop_session(&tool, pid);
        for _ in 0..10 {
            if child.try_wait().ok().flatten().is_some() {
                break;
            }
            thread::sleep(Duration::from_millis(50));
        }
        let _ = child.kill();
        let _ = child.wait();
        let _ = reader.join();
        ctx.request_repaint();
        if failed.is_some() {
            for _ in 0..15 {
                if stop.load(Ordering::Relaxed) || !enabled.load(Ordering::Relaxed) {
                    break;
                }
                thread::sleep(Duration::from_millis(200));
            }
        }
    }
}

pub struct FrameParser {
    pid: u32,
    columns: Option<(usize, usize, usize, usize)>,
    chains: BTreeMap<String, VecDeque<(f64, f64)>>,
}
impl FrameParser {
    pub fn new(pid: u32) -> Self {
        Self {
            pid,
            columns: None,
            chains: BTreeMap::new(),
        }
    }
    pub fn feed(&mut self, line: &str) -> Option<u32> {
        if line.len() > 8192 {
            return None;
        }
        let fields: Vec<_> = line
            .trim()
            .trim_start_matches('\u{feff}')
            .split(',')
            .map(|v| v.trim_matches('"'))
            .collect();
        if fields
            .first()
            .map(|v| v.eq_ignore_ascii_case("Application"))
            == Some(true)
        {
            let index = |name: &str| fields.iter().position(|v| v.eq_ignore_ascii_case(name));
            self.columns = index("ProcessID")
                .zip(index("SwapChainAddress"))
                .zip(index("TimeInSeconds"))
                .zip(index("MsBetweenPresents"))
                .map(|(((a, b), c), d)| (a, b, c, d));
            return None;
        }
        let (a, b, c, d) = self.columns?;
        if fields.get(a)?.parse::<u32>().ok()? != self.pid {
            return None;
        }
        let time = fields.get(c)?.parse::<f64>().ok()?;
        let delta = fields.get(d)?.parse::<f64>().ok()?;
        if !time.is_finite() || time < 0. || !delta.is_finite() || !(0.05..=10000.).contains(&delta)
        {
            return None;
        }
        let key = fields.get(b)?.to_string();
        if !self.chains.contains_key(&key) && self.chains.len() >= 8 {
            return None;
        }
        let samples = self.chains.entry(key).or_default();
        if samples.back().map(|(old, _)| time < *old).unwrap_or(false) {
            samples.clear();
        }
        samples.push_back((time, delta));
        while samples.front().map(|(t, _)| time - t > 1.).unwrap_or(false) || samples.len() > 10000
        {
            samples.pop_front();
        }
        let best = self
            .chains
            .values()
            .filter(|s| s.len() >= 3 && s.back().map(|(t, _)| time - t < 1.).unwrap_or(false))
            .max_by_key(|s| s.len())?;
        let fps = 1000. * best.len() as f64 / best.iter().map(|(_, delta)| delta).sum::<f64>();
        if fps.is_finite() && fps >= 1. && fps <= 20000. {
            Some(fps.round() as u32)
        } else {
            None
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn header() -> &'static str {
        "Application,ProcessID,SwapChainAddress,TimeInSeconds,MsBetweenPresents"
    }
    #[test]
    fn measures_only_selected_process() {
        let mut p = FrameParser::new(7);
        p.feed(header());
        for i in 0..3 {
            assert_eq!(
                p.feed(&format!("Other.exe,8,0x1,{},1", i as f64 / 60.)),
                None
            );
        }
        for i in 0..5 {
            p.feed(&format!(
                "RocketLeague.exe,7,0x1,{},16.6666667",
                i as f64 / 60.
            ));
        }
        assert_eq!(p.feed("RocketLeague.exe,7,0x1,0.1,16.6666667"), Some(60));
    }
    #[test]
    fn rejects_bad_samples() {
        let mut p = FrameParser::new(7);
        p.feed(header());
        for value in ["NaN", "inf", "0", "-1", "500000"] {
            assert_eq!(p.feed(&format!("RocketLeague.exe,7,0x1,1,{value}")), None);
        }
    }
    #[test]
    fn does_not_add_swap_chains_together() {
        let mut p = FrameParser::new(7);
        p.feed(header());
        for i in 0..60 {
            p.feed(&format!(
                "RocketLeague.exe,7,0x1,{},16.6666667",
                i as f64 / 60.
            ));
            if i % 6 == 0 {
                p.feed(&format!("RocketLeague.exe,7,0x2,{},100", i as f64 / 60.));
            }
        }
        assert_eq!(p.feed("RocketLeague.exe,7,0x1,1,16.6666667"), Some(60));
    }
    #[test]
    fn old_values_are_not_live() {
        let r = Reading {
            fps: Some(144),
            updated: Some(Instant::now() - Duration::from_secs(2)),
            ..Default::default()
        };
        assert_eq!(r.live_fps(), None);
    }
    #[test]
    fn accepts_utf8_bom_in_live_header(){
        let mut p=FrameParser::new(7);p.feed(&format!("\u{feff}{}",header()));
        for i in 0..4{p.feed(&format!("RocketLeague.exe,7,0x1,{},4.1666667",i as f64/240.));}
        assert_eq!(p.feed("RocketLeague.exe,7,0x1,0.02,4.1666667"),Some(240));
    }
}
