//! Minimal ZIP reader (central directory + stored/deflate), enough for map archives. No ZIP64.
use super::util::{err, R};
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

pub struct Member {
    pub name: String,
    pub method: u16,
    pub compressed: u64,
    pub size: u64,
    pub offset: u64,
    pub external: u32,
    pub crc: u32,
}

impl Member {
    pub fn is_dir(&self) -> bool { self.name.ends_with('/') }
}

fn u16_at(d: &[u8], o: usize) -> R<u16> { d.get(o..o + 2).map(|b| u16::from_le_bytes([b[0], b[1]])).ok_or("ZIP dañado.".into()) }
fn u32_at(d: &[u8], o: usize) -> R<u32> { d.get(o..o + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]])).ok_or("ZIP dañado.".into()) }

pub fn members(path: &Path) -> R<Vec<Member>> {
    let mut file = std::fs::File::open(path).map_err(err)?;
    let len = file.metadata().map_err(err)?.len();
    let tail_len = len.min(65_557) as usize;
    file.seek(SeekFrom::Start(len - tail_len as u64)).map_err(err)?;
    let mut tail = vec![0u8; tail_len];
    file.read_exact(&mut tail).map_err(err)?;
    let eocd = (0..tail_len.saturating_sub(21)).rev().find(|&i| tail[i..i + 4] == [0x50, 0x4b, 0x05, 0x06]).ok_or("File is not a zip file")?;
    let count = u16_at(&tail, eocd + 10)? as usize;
    let dir_size = u32_at(&tail, eocd + 12)? as u64;
    let dir_offset = u32_at(&tail, eocd + 16)? as u64;
    if count == 0xFFFF || dir_offset == 0xFFFF_FFFF { return Err("ZIP64 no compatible. Extrae el mapa e importa el UDK.".into()); }
    file.seek(SeekFrom::Start(dir_offset)).map_err(err)?;
    let mut dir = vec![0u8; dir_size as usize];
    file.read_exact(&mut dir).map_err(err)?;
    let mut out = Vec::with_capacity(count);
    let mut at = 0;
    for _ in 0..count {
        if u32_at(&dir, at)? != 0x0201_4b50 { return Err("ZIP dañado.".into()); }
        let flags = u16_at(&dir, at + 8)?;
        let name_len = u16_at(&dir, at + 28)? as usize;
        let extra = u16_at(&dir, at + 30)? as usize;
        let comment = u16_at(&dir, at + 32)? as usize;
        let raw = dir.get(at + 46..at + 46 + name_len).ok_or("ZIP dañado.")?;
        // Bit 11: UTF-8 names; otherwise CP437 like Python's zipfile (ASCII in practice).
        let name = if flags & 0x800 != 0 { String::from_utf8_lossy(raw).into_owned() } else { raw.iter().map(|&b| b as char).collect() };
        let member = Member {
            name,
            method: u16_at(&dir, at + 10)?,
            compressed: u32_at(&dir, at + 20)? as u64,
            size: u32_at(&dir, at + 24)? as u64,
            offset: u32_at(&dir, at + 42)? as u64,
            external: u32_at(&dir, at + 38)?,
            crc: u32_at(&dir, at + 16)?,
        };
        if member.compressed == 0xFFFF_FFFF || member.size == 0xFFFF_FFFF || member.offset == 0xFFFF_FFFF { return Err("ZIP64 no compatible. Extrae el mapa e importa el UDK.".into()); }
        out.push(member);
        at += 46 + name_len + extra + comment;
    }
    Ok(out)
}

/// One member's bytes in memory (small files), checked like extract().
pub fn read(path: &Path, member: &Member) -> R<Vec<u8>> {
    let tmp = std::env::temp_dir().join(format!("sp-zip-{}", super::util::uuid_hex()));
    let result = extract(path, member, &tmp).and_then(|_| std::fs::read(&tmp).map_err(err));
    let _ = std::fs::remove_file(&tmp);
    result
}

/// Extract one member to a file, checking its size and CRC.
pub fn extract(path: &Path, member: &Member, dest: &Path) -> R<()> {
    let mut file = std::fs::File::open(path).map_err(err)?;
    file.seek(SeekFrom::Start(member.offset)).map_err(err)?;
    let mut header = [0u8; 30];
    file.read_exact(&mut header).map_err(err)?;
    if u32_at(&header, 0)? != 0x0403_4b50 { return Err("ZIP dañado.".into()); }
    let skip = u16_at(&header, 26)? as i64 + u16_at(&header, 28)? as i64;
    file.seek(SeekFrom::Current(skip)).map_err(err)?;
    let data = file.take(member.compressed);
    let mut reader: Box<dyn Read> = match member.method {
        0 => Box::new(data),
        8 => Box::new(flate2::read::DeflateDecoder::new(data)),
        m => return Err(format!("Compresión ZIP no compatible ({m}).")),
    };
    let mut out = std::io::BufWriter::new(std::fs::File::create(dest).map_err(err)?);
    let mut hash = crc32fast::Hasher::new();
    let mut buf = vec![0u8; 1 << 20];
    let mut total = 0u64;
    loop {
        let n = reader.read(&mut buf).map_err(err)?;
        if n == 0 { break; }
        total += n as u64;
        if total > member.size { return Err("ZIP dañado (tamaño).".into()); }
        hash.update(&buf[..n]);
        std::io::Write::write_all(&mut out, &buf[..n]).map_err(err)?;
    }
    std::io::Write::flush(&mut out).map_err(err)?;
    if total != member.size { return Err("ZIP dañado (tamaño).".into()); }
    if member.crc != hash.finalize() { return Err("ZIP dañado (CRC).".into()); }
    Ok(())
}
