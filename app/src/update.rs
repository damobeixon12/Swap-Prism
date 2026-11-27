use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

pub fn install_after_exit(installer:&Path,root:&Path,expected:&str)->Result<(),String>{
    use std::os::windows::process::CommandExt;
    let digest=format!("{:x}",Sha256::digest(fs::read(installer).map_err(|e|e.to_string())?));
    if expected.len()!=64||digest!=expected.to_ascii_lowercase(){return Err("El instalador ya no coincide con la descarga verificada.".into());}
    let source=root.join("_engine/SWAPPRISMUpdate.exe");
    let helper=root.join("data/updates/SWAPPRISMUpdate.exe");
    let _=fs::remove_file(root.join("data/updates/runner-ready.json"));
    fs::copy(source,&helper).map_err(|e|format!("Falta el lanzador de actualizaciones: {e}"))?;
    std::process::Command::new(helper).arg("--install").arg(std::process::id().to_string())
        .arg(installer).arg(root).arg(expected).creation_flags(0x08000000)
        .spawn().map_err(|e|e.to_string())?;
    Ok(())
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Release {
    pub version: String,
    pub url: String,
    pub sha256: String,
    pub notes: Option<String>,
}
pub fn validate(release: &Release, current: &str) -> Result<bool, String> {
    let new = semver::Version::parse(&release.version).map_err(|e| e.to_string())?;
    let old = semver::Version::parse(current).map_err(|e| e.to_string())?;
    if !release.url.starts_with("https://")
        || release.url.contains('@')
        || !release
            .url
            .split('?')
            .next()
            .unwrap_or("")
            .to_ascii_lowercase()
            .ends_with(".exe")
    {
        return Err("La descarga debe ser un instalador HTTPS sin credenciales en la URL.".into());
    }
    if release.sha256.len() != 64 || !release.sha256.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("SHA-256 de actualización no válido.".into());
    }
    Ok(new > old && new.pre.is_empty())
}
fn agent() -> Result<ureq::Agent, String> {
    let tls = native_tls::TlsConnector::new().map_err(|e| e.to_string())?;
    Ok(ureq::AgentBuilder::new()
        .redirects(0)
        .timeout_connect(Duration::from_secs(10))
        .timeout_read(Duration::from_secs(60))
        .tls_connector(Arc::new(tls))
        .build())
}
fn get(url: &str) -> Result<ureq::Response, String> {
    let client = agent()?;
    let mut next = url.to_owned();
    for _ in 0..6 {
        if !next.starts_with("https://") || next.contains('@') {
            return Err("La descarga requiere HTTPS en todos sus saltos.".into());
        }
        let response = client.get(&next).call().map_err(|e|match e{ureq::Error::Status(404,_)=>"HTTP 404".to_string(),other=>other.to_string()})?;
        if [301, 302, 303, 307, 308].contains(&response.status()) {
            next = response
                .header("Location")
                .ok_or("Redirección sin destino")?
                .to_owned();
            continue;
        }
        if response.status() != 200 {
            return Err(format!("Respuesta HTTP inesperada: {}", response.status()));
        }
        return Ok(response);
    }
    Err("Demasiadas redirecciones de descarga.".into())
}
pub fn check(url: &str, current: &str) -> Result<Option<Release>, String> {
    if url.is_empty() {
        return Ok(None);
    }
    if !url.starts_with("https://") || url.contains('@') {
        return Err("El manifiesto de actualización requiere HTTPS.".into());
    }
    let response = match get(url){Ok(v)=>v,Err(e) if e=="HTTP 404"&&url.starts_with("https://github.com/")&&url.ends_with("/releases/latest/download/update.json")=>return Ok(None),Err(e)=>return Err(e)};
    let mut bytes = Vec::new();
    response
        .into_reader()
        .take(65537)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > 65536 {
        return Err("Manifiesto demasiado grande.".into());
    }
    let release: Release = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    Ok(if validate(&release, current)? {
        Some(release)
    } else {
        None
    })
}
pub fn download_progress(release:&Release,root:&Path,mut progress:impl FnMut(u64,Option<u64>))->Result<PathBuf,String>{
    if !validate(release, env!("CARGO_PKG_VERSION"))?{return Err("Esta versión no es una actualización estable más reciente.".into());}
    let dir = root.join("data/updates");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join("Swap Prism-Setup.download");
    let result = (|| {
        let response = get(&release.url)?;
        let expected=response.header("Content-Length").and_then(|s|s.parse::<u64>().ok()).filter(|n|*n>0);
        progress(0,expected);
        let mut reader = response.into_reader();
        let mut file = fs::File::create(&path).map_err(|e| e.to_string())?;
        let mut hash = Sha256::new();
        let mut buffer = [0u8; 65536];
        let mut total = 0u64;
        loop {
            let n = reader.read(&mut buffer).map_err(|e| e.to_string())?;
            if n == 0 {
                break;
            }
            total += n as u64;
            progress(total,expected);
            if total > 1024 * 1024 * 1024 {
                return Err("Instalador demasiado grande.".into());
            }
            hash.update(&buffer[..n]);
            file.write_all(&buffer[..n]).map_err(|e| e.to_string())?;
        }
        file.sync_all().map_err(|e| e.to_string())?;
        if format!("{:x}", hash.finalize()) != release.sha256.to_ascii_lowercase() {
            return Err("La descarga no coincide con el SHA-256 publicado. No se ejecuta.".into());
        }
        drop(file);
        let final_path = dir.join("Swap Prism-Setup.exe");
        fs::rename(&path, &final_path).map_err(|e| e.to_string())?;
        Ok(final_path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(path);
    }
    result
}
#[cfg(test)]
mod tests {
    use super::*;
    fn release() -> Release {
        Release {
            version: "0.11.0".into(),
            url: "https://example.com/SWAPPRISM-Setup.exe".into(),
            sha256: "a".repeat(64),
            notes: None,
        }
    }
    #[test]
    fn newer_only() {
        let mut r = release();
        assert!(validate(&r, "0.10.0").unwrap());
        r.version = "0.9.0".into();
        assert!(!validate(&r, "0.10.0").unwrap());
        r.version = "0.11.0-beta.1".into();
        assert!(!validate(&r, "0.10.0").unwrap());
    }
    #[test]
    fn refuses_insecure_or_unverified() {
        let mut r = release();
        r.url = "http://example.com/setup.exe".into();
        assert!(validate(&r, "0.10.0").is_err());
        r = release();
        r.sha256 = "wrong".into();
        assert!(validate(&r, "0.10.0").is_err());
    }
    #[test]
    fn changed_installer_is_not_launched(){
        let root=std::env::temp_dir().join(format!("swapprism-update-check-{}",std::process::id()));
        fs::create_dir_all(root.join("data/updates")).unwrap();
        let file=root.join("data/updates/Swap Prism-Setup.exe");fs::write(&file,b"changed").unwrap();
        assert!(install_after_exit(&file,&root,&"0".repeat(64)).unwrap_err().contains("no coincide"));
        fs::remove_dir_all(root).unwrap();
    }
}
