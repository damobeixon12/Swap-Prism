//! Build tool (replaces the old PowerShell and Python scripts). Run through scripts\swapprism-build.cmd:
//!   package [--skip-build] [--skip-tests]        pruebas, compilación, miniaturas, distribución e instalador NSIS
//!   release [--repo o/r] [--notes "…"] [--skip-build]   prepara outputs/Swap Prism-Update.zip (no publica nada)
//!   check-installer [--keep]                     instala en work/installer-check y prueba el arranque
use serde_json::{json, Value};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

type R<T> = Result<T, String>;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let root = project_root();
    let flag = |name: &str| args.iter().any(|a| a == name);
    let option = |name: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned();
    let result = match args.first().map(String::as_str) {
        Some("package") => package(&root, flag("--skip-build"), flag("--skip-tests")),
        Some("release") => release(
            &root,
            option("--repo").unwrap_or_else(|| "damobeixon12/Swap-Prism".into()),
            option("--notes").unwrap_or_else(|| "Mejoras y correcciones de Swap Prism.".into()),
            flag("--skip-build"),
        ),
        Some("check-installer") => check_installer(&root, flag("--keep")),
        _ => Err("Uso: swapprism-build package|release|check-installer [opciones]".into()),
    };
    if let Err(e) = result {
        eprintln!("ERROR: {e}");
        std::process::exit(1);
    }
}

/// The folder that holds app/ and scripts/, found from the executable (app/target/release/…) or the current folder.
fn project_root() -> PathBuf {
    let candidates = std::env::current_exe().ok().into_iter().flat_map(|p| p.ancestors().map(Path::to_path_buf).collect::<Vec<_>>());
    candidates
        .chain(std::env::current_dir().ok())
        .find(|p| p.join("app/Cargo.toml").is_file() && p.join("scripts").is_dir())
        .expect("No se encuentra la carpeta del proyecto.")
}

fn version(root: &Path) -> R<String> {
    let toml = read_text(&root.join("app/Cargo.toml"))?;
    toml.lines()
        .find(|l| l.starts_with("version"))
        .and_then(|l| l.split('"').nth(1))
        .map(str::to_string)
        .ok_or_else(|| "Falta la versión en app/Cargo.toml.".into())
}

fn read_text(path: &Path) -> R<String> {
    std::fs::read_to_string(path).map(|s| s.trim_start_matches('\u{feff}').to_string()).map_err(|e| format!("{}: {e}", path.display()))
}

fn write(path: &Path, data: impl AsRef<[u8]>) -> R<()> {
    std::fs::write(path, data).map_err(|e| format!("{}: {e}", path.display()))
}

fn copy(from: &Path, to: &Path) -> R<()> {
    std::fs::copy(from, to).map(|_| ()).map_err(|e| format!("{} -> {}: {e}", from.display(), to.display()))
}

fn run(command: &mut Command, failure: &str) -> R<()> {
    let status = command.status().map_err(|e| format!("{failure} ({e})"))?;
    if status.success() { Ok(()) } else { Err(failure.into()) }
}

fn cargo_home() -> PathBuf {
    std::env::var_os("CARGO_HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(std::env::var_os("USERPROFILE").unwrap_or_default()).join(".cargo"))
}

/// First `registry/src/<index>/<crate dir starting with prefix>/<relative>` that exists.
fn registry_file(prefix: &str, relative: &str) -> Option<PathBuf> {
    let mut found: Vec<PathBuf> = std::fs::read_dir(cargo_home().join("registry/src")).ok()?
        .flatten()
        .flat_map(|index| std::fs::read_dir(index.path()).into_iter().flatten().flatten())
        .filter(|krate| krate.file_name().to_string_lossy().starts_with(prefix))
        .map(|krate| krate.path().join(relative))
        .filter(|p| p.is_file())
        .collect();
    found.sort();
    found.pop()
}

fn sha256_hex(path: &Path) -> R<String> {
    use sha2::{Digest, Sha256};
    let mut file = std::fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut hasher = Sha256::new();
    std::io::copy(&mut file, &mut hasher).map_err(|e| e.to_string())?;
    Ok(hasher.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

// ---------- package ----------

fn package(root: &Path, skip_build: bool, skip_tests: bool) -> R<()> {
    let version = version(root)?;
    let release = root.join("app/target/release");
    let webview = registry_file("webview2-com-sys-", "x64/WebView2Loader.dll").ok_or("Falta WebView2Loader.dll (cargo fetch).")?;
    if !skip_build {
        println!("== Rust");
        let path = format!("{};{}", webview.parent().unwrap().display(), std::env::var("PATH").unwrap_or_default());
        let cargo = |args: &[&str]| { let mut c = Command::new("cargo"); c.args(args).current_dir(root.join("app")).env("PATH", &path); c };
        if !skip_tests { run(&mut cargo(&["test", "--release", "--locked"]), "Fallan las pruebas.")?; }
        // Only the shipped binaries: this tool is running and Windows cannot replace its .exe.
        run(&mut cargo(&["build", "--release", "--locked", "--bin", "SWAPPRISM", "--bin", "SWAPPRISMUpdate"]), "La compilación Rust no se completó.")?;
        copy(&webview, &release.join("WebView2Loader.dll"))?;
    }
    println!("== Miniaturas");
    thumbnails(root)?;
    println!("== Distribución");
    let dist = root.join("work/distribution");
    std::fs::create_dir_all(&dist).map_err(|e| e.to_string())?;
    let exe = dist.join("Swap Prism.exe");
    copy(&release.join("SWAPPRISM.exe"), &exe)?;
    copy(&release.join("WebView2Loader.dll"), &dist.join("WebView2Loader.dll"))?;
    write(&dist.join("THIRD_PARTY.txt"), read_text(&root.join("app/assets/THIRD_PARTY.txt"))?.replace("{VERSION}", &version))?;
    embed_icon(&exe, &root.join("app/assets/swapprism.ico"))?;
    // _engine keeps only the native updater and the licences (the engine runs inside Swap Prism.exe).
    let engine = dist.join("_engine");
    if engine.exists() { std::fs::remove_dir_all(&engine).map_err(|e| e.to_string())?; }
    let licenses = engine.join("licenses");
    std::fs::create_dir_all(&licenses).map_err(|e| e.to_string())?;
    copy(&release.join("SWAPPRISMUpdate.exe"), &engine.join("SWAPPRISMUpdate.exe"))?;
    for entry in std::fs::read_dir(root.join("app/assets/licenses")).map_err(|e| e.to_string())?.flatten() {
        copy(&entry.path(), &licenses.join(entry.file_name()))?;
    }
    copy(&root.join("app/assets/icons/LICENSE"), &licenses.join("Lucide-LICENSE.txt"))?;
    copy(&root.join("app/assets/fonts/Nunito-OFL.txt"), &licenses.join("Nunito-OFL.txt"))?;
    copy(&registry_file("wry-0.57.3", "LICENSE-MIT").ok_or("Falta la licencia de Wry.")?, &licenses.join("Wry-MIT.txt"))?;
    installer_payload(root)?;
    println!("== Instalador");
    run(Command::new(root.join("work/toolchain/nsis-3.11/makensis.exe")).arg("/V2").arg("app/installer.nsi").current_dir(root), "No se pudo generar el instalador.")?;
    println!("SHA256 {}  outputs/Swap Prism-Setup.exe", sha256_hex(&root.join("outputs/Swap Prism-Setup.exe"))?.to_uppercase());
    Ok(())
}

/// 128 px WebP thumbnails for the installer from the app's cache (outputs/LocalSkins/data/thumbnails, read only).
/// work/package-thumbnails is an incremental cache: only new images are converted.
fn thumbnails(root: &Path) -> R<()> {
    const MAX: u32 = 128;
    let source = root.join("outputs/LocalSkins/data/thumbnails");
    let target = root.join("work/package-thumbnails");
    if !source.is_dir() { return Err(format!("No existe {}: abre la app al menos una vez para crear la caché de miniaturas.", source.display())); }
    std::fs::create_dir_all(&target).map_err(|e| e.to_string())?;
    let mut pngs: Vec<PathBuf> = std::fs::read_dir(&source).map_err(|e| e.to_string())?.flatten().map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x.eq_ignore_ascii_case("png"))).collect();
    pngs.sort();
    let convert = |png: &PathBuf| -> R<(u64, u64)> {
        let out = target.join(png.file_stem().unwrap()).with_extension("webp");
        if !out.exists() {
            let mut image = image::open(png).map_err(|e| format!("{}: {e}", png.display()))?;
            if image.width().max(image.height()) > MAX { image = image.resize(MAX, MAX, image::imageops::FilterType::Lanczos3); }
            let rgba = image.to_rgba8();
            let mut bytes = Vec::new();
            // shortcut: the image crate only writes lossless WebP (the old cache is lossy q82), new thumbnails are ~2x bigger; fine while few are added.
            image::codecs::webp::WebPEncoder::new_lossless(&mut bytes)
                .encode(&rgba, rgba.width(), rgba.height(), image::ExtendedColorType::Rgba8).map_err(|e| e.to_string())?;
            write(&out, bytes)?;
        }
        let (w, h) = image::image_dimensions(&out).map_err(|e| format!("Miniatura inválida: {} ({e})", out.display()))?;
        let mut magic = [0u8; 12];
        std::io::Read::read_exact(&mut std::fs::File::open(&out).map_err(|e| e.to_string())?, &mut magic).map_err(|e| e.to_string())?;
        if &magic[8..] != b"WEBP" || w.max(h) > MAX { return Err(format!("Miniatura inválida: {}", out.display())); }
        Ok((png.metadata().map_err(|e| e.to_string())?.len(), out.metadata().map_err(|e| e.to_string())?.len()))
    };
    let chunk = pngs.len().div_ceil(5).max(1);
    let results: Vec<R<(u64, u64)>> = std::thread::scope(|s| {
        let handles: Vec<_> = pngs.chunks(chunk).map(|part| s.spawn(move || part.iter().map(convert).collect::<Vec<_>>())).collect();
        handles.into_iter().flat_map(|h| h.join().unwrap()).collect()
    });
    let sizes = results.into_iter().collect::<R<Vec<_>>>()?;
    println!("{}", json!({"count": sizes.len(), "source_bytes": sizes.iter().map(|s| s.0).sum::<u64>(), "bundled_bytes": sizes.iter().map(|s| s.1).sum::<u64>()}));
    Ok(())
}

/// Writes the .ico images into the executable as Windows icon resources (RT_ICON 3 + RT_GROUP_ICON 14).
fn embed_icon(exe: &Path, ico: &Path) -> R<()> {
    use windows::core::PCWSTR;
    use windows::Win32::System::LibraryLoader::{BeginUpdateResourceW, EndUpdateResourceW, UpdateResourceW};
    let data = std::fs::read(ico).map_err(|e| e.to_string())?;
    let u16_at = |i: usize| u16::from_le_bytes([data[i], data[i + 1]]);
    let u32_at = |i: usize| u32::from_le_bytes(data[i..i + 4].try_into().unwrap());
    if data.len() < 6 || u16_at(0) != 0 || u16_at(2) != 1 { return Err("ICO inválido".into()); }
    let count = u16_at(4) as usize;
    let wide: Vec<u16> = exe.as_os_str().encode_wide_null();
    let int = |id: usize| PCWSTR(id as *const u16);
    unsafe {
        let handle = BeginUpdateResourceW(PCWSTR(wide.as_ptr()), false).map_err(|e| e.to_string())?;
        let result = (|| -> R<()> {
            let mut group = Vec::from([0u8, 0, 1, 0]);
            group.extend((count as u16).to_le_bytes());
            for index in 0..count {
                let entry = data.get(6 + index * 16..22 + index * 16).ok_or("ICO inválido")?;
                let (size, offset) = (u32_at(6 + index * 16 + 8) as usize, u32_at(6 + index * 16 + 12) as usize);
                let image = data.get(offset..offset + size).ok_or("ICO inválido")?;
                UpdateResourceW(handle, int(3), int(index + 1), 0, Some(image.as_ptr().cast()), image.len() as u32).map_err(|e| e.to_string())?;
                group.extend(&entry[..12]);
                group.extend(((index + 1) as u16).to_le_bytes());
            }
            UpdateResourceW(handle, int(14), int(1), 0, Some(group.as_ptr().cast()), group.len() as u32).map_err(|e| e.to_string())
        })();
        if result.is_err() { let _ = EndUpdateResourceW(handle, true); return result; }
        EndUpdateResourceW(handle, false).map_err(|e| e.to_string())
    }
}

trait EncodeWideNull { fn encode_wide_null(&self) -> Vec<u16>; }
impl EncodeWideNull for std::ffi::OsStr {
    fn encode_wide_null(&self) -> Vec<u16> { use std::os::windows::ffi::OsStrExt; self.encode_wide().chain(Some(0)).collect() }
}

/// work/installer-payload.nsh: the File commands, with progress weighted by bytes of completed files.
fn installer_payload(root: &Path) -> R<()> {
    let dist = root.join("work/distribution");
    let mut files: Vec<(PathBuf, String, bool)> = ["Swap Prism.exe", "WebView2Loader.dll", "THIRD_PARTY.txt"].iter()
        .map(|n| (dist.join(n), n.to_string(), false)).collect();
    files.insert(2, (root.join("app/assets/swapprism.ico"), "swapprism.ico".into(), false));
    let mut engine = Vec::new();
    walk(&dist.join("_engine"), &mut engine)?;
    engine.sort_by_key(|p| p.to_string_lossy().to_lowercase());
    for path in engine {
        let relative = path.strip_prefix(dist.join("_engine")).unwrap().to_string_lossy().replace('\\', "/");
        files.push((path, format!("_engine/{relative}"), false));
    }
    for name in ["PresentMon.exe", "PresentMon-LICENSE.txt"] { files.push((root.join("app/vendor").join(name), format!("_telemetry/{name}"), false)); }
    let mut thumbs: Vec<PathBuf> = std::fs::read_dir(root.join("work/package-thumbnails")).map_err(|e| e.to_string())?.flatten().map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "webp")).collect();
    thumbs.sort();
    for p in thumbs { let name = p.file_name().unwrap().to_string_lossy().to_string(); files.push((p, format!("data/thumbnails/{name}"), true)); }
    files.push((root.join("app/assets/settings.json"), "data/settings.json".into(), true));
    let sizes = files.iter().map(|(p, _, _)| p.metadata().map(|m| m.len()).map_err(|e| format!("{}: {e}", p.display()))).collect::<R<Vec<u64>>>()?;
    let total: u64 = sizes.iter().sum();
    let escape = |s: &str| s.replace('$', "$$").replace('"', "$\\\"");
    let (mut done, mut percent, mut lines) = (0u64, -1i64, Vec::new());
    let (mut last_dir, mut last_keep) = (None::<String>, None::<bool>);
    for ((source, target, keep), size) in files.iter().zip(&sizes) {
        let parent = target.rsplit_once('/').map(|(d, _)| format!("\\{}", d.replace('/', "\\"))).unwrap_or_default();
        if last_dir.as_ref() != Some(&parent) { lines.push(format!("SetOutPath \"$INSTDIR{parent}\"")); last_dir = Some(parent); }
        if last_keep != Some(*keep) { lines.push(format!("SetOverwrite {}", if *keep { "off" } else { "on" })); last_keep = Some(*keep); }
        let name = target.rsplit('/').next().unwrap();
        lines.push(format!("File \"/oname={}\" \"{}\"", escape(name), escape(&source.to_string_lossy().replace('/', "\\"))));
        done += size;
        let now = (done as f64 / total as f64 * 98.0).round() as i64;
        if now > percent { lines.push(format!("!insertmacro UpdateProgress {now}")); percent = now; }
    }
    lines.push("SetOverwrite on".into());
    write(&root.join("work/installer-payload.nsh"), lines.join("\r\n") + "\r\n")?;
    println!("Installer payload files {} unpacked bytes {total}", files.len());
    Ok(())
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) -> R<()> {
    for entry in std::fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))?.flatten() {
        let path = entry.path();
        if path.is_dir() { walk(&path, out)?; } else { out.push(path); }
    }
    Ok(())
}

// ---------- release ----------

/// Replaces, in every occurrence of `prefix`, the text up to the next quote.
fn replace_after(text: &str, prefix: &str, value: &str) -> R<String> {
    if !text.contains(prefix) { return Err(format!("No se encuentra {prefix}")); }
    let mut out = String::new();
    let mut rest = text;
    while let Some(i) = rest.find(prefix) {
        let start = i + prefix.len();
        let end = start + rest[start..].find('"').ok_or("Comillas sin cerrar")?;
        out.push_str(&rest[..start]);
        out.push_str(value);
        rest = &rest[end..];
    }
    out.push_str(rest);
    Ok(out)
}

fn release(root: &Path, repository: String, notes: String, skip_build: bool) -> R<()> {
    let mut repository = repository.trim().replace("https://github.com/", "").trim_end_matches('/').to_string();
    if let Some(r) = repository.strip_suffix(".git") { repository = r.into(); }
    let valid = |s: &str| !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || "_.-".contains(c));
    if !repository.split_once('/').is_some_and(|(a, b)| valid(a) && valid(b)) { return Err("Usa propietario/repositorio o el enlace HTTPS de GitHub.".into()); }
    let version = version(root)?;
    let parts: Vec<&str> = version.split('.').collect();
    if parts.len() != 3 || !parts.iter().all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit())) { return Err("Pon una versión estable x.y.z en app/Cargo.toml.".into()); }
    let channel = format!("https://github.com/{repository}/releases/latest/download/update.json");
    let settings_path = root.join("app/assets/settings.json");
    let mut settings: Value = serde_json::from_str(&read_text(&settings_path)?).map_err(|e| e.to_string())?;
    if skip_build && settings["update_manifest_url"] != channel { return Err("El canal cambió. Ejecuta sin --skip-build para incorporarlo al instalador.".into()); }
    // Files are rewritten only when a value changes, so a repeated release leaves git clean.
    if settings["update_manifest_url"] != channel || settings["release_repository"] != repository.as_str() {
        settings["update_manifest_url"] = channel.clone().into();
        settings["release_repository"] = repository.clone().into();
        write(&settings_path, serde_json::to_string_pretty(&settings).unwrap())?;
    }
    let nsi_path = root.join("app/installer.nsi");
    let old_nsi = read_text(&nsi_path)?;
    let mut nsi = replace_after(&old_nsi, "VIProductVersion \"", &format!("{version}.0"))?;
    nsi = replace_after(&nsi, "VIAddVersionKey \"FileVersion\" \"", &version)?;
    nsi = replace_after(&nsi, "\"DisplayVersion\" \"", &version)?;
    if nsi != old_nsi { write(&nsi_path, [b"\xef\xbb\xbf".as_slice(), nsi.as_bytes()].concat())?; } // NSIS needs the BOM to read UTF-8
    let lock_path = root.join("app/Cargo.lock");
    let lock = read_text(&lock_path)?;
    let at = lock.find("name = \"swapprism\"").ok_or("Falta swapprism en Cargo.lock")?;
    let (head, tail) = lock.split_at(at);
    let new_lock = format!("{head}{}", replace_after_once(tail, "version = \"", &version)?);
    if new_lock != lock { write(&lock_path, new_lock)?; }
    if !skip_build { package(root, false, false)?; }
    let setup = root.join("outputs/Swap Prism-Setup.exe");
    let built = file_version(&std::fs::read(&setup).map_err(|_| "Falta el instalador.")?).unwrap_or_default();
    if built != version { return Err(format!("El instalador es {built} y el proyecto {version}. Vuelve a compilar.")); }
    let asset = "Swap-Prism-Setup.exe";
    let manifest = json!({"version": version, "url": format!("https://github.com/{repository}/releases/download/v{version}/{asset}"),
        "sha256": sha256_hex(&setup)?, "notes": notes});
    let manifest_text = serde_json::to_string_pretty(&manifest).unwrap();
    write(&root.join("outputs/update.json"), &manifest_text)?;
    write(&root.join("outputs/PUBLICAR-ACTUALIZACION.txt"), format!("SWAP PRISM {version} - ARCHIVOS PREPARADOS, SIN PUBLICAR\r\n\r\n\
1. Extrae Swap Prism-Update.zip fuera de la carpeta del proyecto.\r\n\
2. Abre https://github.com/{repository}/releases/new\r\n\
3. Etiqueta: v{version}. Titulo: Swap Prism {version}.\r\n\
4. Sube los dos archivos extraidos: {asset} y update.json.\r\n\
5. Publica como version estable y marca Set as latest release.\r\n\r\n\
El canal de las apps es {channel}\r\n\
Actualizar el codigo del repo NO publica un instalador: repite este proceso.\r\n\
Para la siguiente version cambia primero version en app/Cargo.toml y ejecuta\r\n\
Preparar update.cmd. No subas outputs/LocalSkins/data ni backups al repositorio.\r\n"))?;
    let setup_bytes = std::fs::read(&setup).map_err(|e| e.to_string())?;
    write_zip(&root.join("outputs/Swap Prism-Update.zip"), &[(asset, &setup_bytes), ("update.json", manifest_text.as_bytes())])?;
    println!("Listo: outputs/Swap Prism-Update.zip · {version}");
    println!("No se ha subido ni publicado nada.");
    Ok(())
}

fn replace_after_once(text: &str, prefix: &str, value: &str) -> R<String> {
    let start = text.find(prefix).ok_or_else(|| format!("No se encuentra {prefix}"))? + prefix.len();
    let end = start + text[start..].find('"').ok_or("Comillas sin cerrar")?;
    Ok(format!("{}{value}{}", &text[..start], &text[end..]))
}

/// The FileVersion string of the version resource (UTF-16 "FileVersion", padding, value).
fn file_version(exe: &[u8]) -> Option<String> {
    let key: Vec<u8> = "FileVersion\0".encode_utf16().flat_map(u16::to_le_bytes).collect();
    let at = exe.windows(key.len()).position(|w| w == key.as_slice())? + key.len();
    let units: Vec<u16> = exe[at..].chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).skip_while(|&u| u == 0).take_while(|&u| u != 0).collect();
    Some(String::from_utf16_lossy(&units))
}

/// Minimal deflate ZIP writer (fixed 1980-01-01 timestamp).
fn write_zip(path: &Path, entries: &[(&str, &[u8])]) -> R<()> {
    let mut out = Vec::new();
    let mut central = Vec::new();
    for (name, data) in entries {
        let mut encoder = flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::new(6));
        encoder.write_all(data).map_err(|e| e.to_string())?;
        let packed = encoder.finish().map_err(|e| e.to_string())?;
        let crc = crc32fast::hash(data);
        let offset = out.len() as u32;
        let common = |v: &mut Vec<u8>| {
            for x in [20u16, 0, 8, 0, 0x21] { v.extend(x.to_le_bytes()); } // version, flags, deflate, time, date
            for x in [crc, packed.len() as u32, data.len() as u32] { v.extend(x.to_le_bytes()); }
            v.extend((name.len() as u16).to_le_bytes());
            v.extend(0u16.to_le_bytes()); // extra
        };
        out.extend(0x04034b50u32.to_le_bytes());
        common(&mut out);
        out.extend(name.as_bytes());
        out.extend(&packed);
        central.extend(0x02014b50u32.to_le_bytes());
        central.extend(20u16.to_le_bytes());
        common(&mut central);
        for x in [0u16, 0, 0] { central.extend(x.to_le_bytes()); } // comment, disk, internal attributes
        central.extend(0u32.to_le_bytes());
        central.extend(offset.to_le_bytes());
        central.extend(name.as_bytes());
    }
    let start = out.len() as u32;
    out.extend(&central);
    out.extend(0x06054b50u32.to_le_bytes());
    for x in [0u16, 0, entries.len() as u16, entries.len() as u16] { out.extend(x.to_le_bytes()); }
    for x in [central.len() as u32, start] { out.extend(x.to_le_bytes()); }
    out.extend(0u16.to_le_bytes());
    write(path, out)
}

// ---------- check-installer ----------

fn check_installer(root: &Path, keep: bool) -> R<()> {
    let work = root.join("work");
    let install = work.join("installer-check");
    if install.exists() { return Err("work/installer-check ya existe; bórrala antes de repetir la prueba.".into()); }
    run(Command::new(root.join("outputs/Swap Prism-Setup.exe")).args(["/S", "/TESTMODE"]).arg(format!("/D={}", install.display())), "Falló el instalador (¿Swap Prism abierto? Ciérralo con Salir).")?;
    let exe = install.join("Swap Prism.exe");
    if sha256_hex(&exe)? != sha256_hex(&work.join("distribution/Swap Prism.exe"))? { return Err("El instalador contiene otro ejecutable.".into()); }
    for private in ["data/profile.json", "data/profile-draft.json", "data/profile/images", "data/backups", "data/tracker.json", "data/tracker-browser", "data/rank-draft.json"] {
        if install.join(private).exists() { return Err(format!("El instalador contiene datos privados: {private}")); }
    }
    if install.join("_engine/SWAPPRISMEngine.exe").exists() { return Err("El instalador aún lleva el motor Python.".into()); }
    let log = work.join("installer-smoke.json");
    let _ = std::fs::remove_file(&log);
    let mut app = Command::new(&exe).arg("--smoke").arg(&log).spawn().map_err(|e| e.to_string())?;
    let started = std::time::Instant::now();
    while app.try_wait().map_err(|e| e.to_string())?.is_none() {
        if started.elapsed().as_secs() > 30 { let _ = app.kill(); return Err("La prueba de arranque no terminó.".into()); }
        std::thread::sleep(std::time::Duration::from_millis(200));
    }
    let report: Value = serde_json::from_str(&read_text(&log)?).map_err(|e| e.to_string())?;
    let version = version(root)?;
    if report["version"] != version.as_str() || report["gui"] != "ok" || report["engine"] != "native" {
        return Err(format!("Falló el arranque instalado: {report}"));
    }
    if !keep {
        let meta = std::fs::symlink_metadata(&install).map_err(|e| e.to_string())?;
        if meta.file_type().is_symlink() { return Err("Destino temporal cambiado; no se borra.".into()); }
        std::fs::remove_dir_all(&install).map_err(|e| e.to_string())?;
    }
    println!("{}", json!({"Version": version, "InstallerStart": "ok", "Engine": "native", "PrivateDataIncluded": false, "TemporaryInstallRemoved": !keep}));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zip_round_trips_through_the_engine_reader() {
        let dir = std::env::temp_dir().join(format!("sp-build-zip-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("a.zip");
        let big = vec![7u8; 100_000];
        write_zip(&path, &[("x.bin", &big), ("update.json", b"{}")]).unwrap();
        let out = Command::new("powershell").args(["-NoProfile", "-Command", &format!("Add-Type -A System.IO.Compression.FileSystem; [IO.Compression.ZipFile]::OpenRead('{}').Entries | % {{ $_.FullName + ':' + $_.Length }}", path.display())]).output().unwrap();
        std::fs::remove_dir_all(&dir).unwrap();
        assert_eq!(String::from_utf8_lossy(&out.stdout).split_whitespace().collect::<Vec<_>>(), ["x.bin:100000", "update.json:2"]);
    }

    #[test]
    fn replaces_quoted_values() {
        let nsi = "VIProductVersion \"0.1.0.0\"\nWriteRegStr \"DisplayVersion\" \"0.1.0\"\n\"DisplayVersion\" \"0.1.0\"";
        let out = replace_after(nsi, "\"DisplayVersion\" \"", "1.2.3").unwrap();
        assert_eq!(out.matches("1.2.3").count(), 2);
        assert_eq!(replace_after(&out, "VIProductVersion \"", "1.2.3.0").unwrap().lines().next(), Some("VIProductVersion \"1.2.3.0\""));
    }

    #[test]
    fn reads_the_file_version_resource() {
        let mut exe = b"MZ....".to_vec();
        exe.extend("FileVersion\0\0".encode_utf16().flat_map(u16::to_le_bytes));
        exe.extend("0.31.6\0".encode_utf16().flat_map(u16::to_le_bytes));
        assert_eq!(file_version(&exe).as_deref(), Some("0.31.6"));
    }
}
