//! DXT1/DXT5 (BC1/BC3) encode and decode for inline texture mips, plus DDS reading and PIL-like resizing.
use super::util::{err, R};
use image::{imageops::FilterType, RgbaImage};

fn to565(c: [f32; 3]) -> u16 {
    let q = |v: f32, max: f32| (v.clamp(0., 255.) * max / 255. + 0.5) as u16;
    (q(c[0], 31.) << 11) | (q(c[1], 63.) << 5) | q(c[2], 31.)
}
fn from565(c: u16) -> [u8; 3] {
    let (r, g, b) = ((c >> 11) & 31, (c >> 5) & 63, c & 31);
    [((r << 3) | (r >> 2)) as u8, ((g << 2) | (g >> 4)) as u8, ((b << 3) | (b >> 2)) as u8]
}

fn block_pixels(img: &RgbaImage, bx: u32, by: u32) -> [[u8; 4]; 16] {
    let mut out = [[0u8; 4]; 16];
    for y in 0..4 { for x in 0..4 {
        let (px, py) = ((bx * 4 + x).min(img.width() - 1), (by * 4 + y).min(img.height() - 1));
        out[(y * 4 + x) as usize] = img.get_pixel(px, py).0;
    } }
    out
}

/// Colour endpoints along the block's principal axis, then the nearest of the four palette entries.
fn encode_color(pixels: &[[u8; 4]; 16]) -> [u8; 8] {
    let mean = (0..3).map(|c| pixels.iter().map(|p| p[c] as f32).sum::<f32>() / 16.).collect::<Vec<_>>();
    let mut cov = [[0f32; 3]; 3];
    for p in pixels { for i in 0..3 { for j in 0..3 { cov[i][j] += (p[i] as f32 - mean[i]) * (p[j] as f32 - mean[j]); } } }
    let mut axis = [1f32, 1., 1.];
    for _ in 0..8 {
        let next = [0, 1, 2].map(|i| (0..3).map(|j| cov[i][j] * axis[j]).sum::<f32>());
        let len = next.iter().map(|v| v * v).sum::<f32>().sqrt();
        if len < 1e-6 { break; }
        axis = next.map(|v| v / len);
    }
    let proj = |p: &[u8; 4]| (0..3).map(|i| (p[i] as f32 - mean[i]) * axis[i]).sum::<f32>();
    let (lo, hi) = pixels.iter().map(proj).fold((f32::MAX, f32::MIN), |(a, b), v| (a.min(v), b.max(v)));
    let at = |t: f32| [0, 1, 2].map(|i| mean[i] + axis[i] * t);
    let (mut c0, mut c1) = (to565(at(hi)), to565(at(lo)));
    if c0 < c1 { std::mem::swap(&mut c0, &mut c1); }
    let mut out = [0u8; 8];
    out[0..2].copy_from_slice(&c0.to_le_bytes());
    out[2..4].copy_from_slice(&c1.to_le_bytes());
    if c0 == c1 { return out; }
    let (a, b) = (from565(c0), from565(c1));
    let palette: [[f32; 3]; 4] = [a.map(f32::from), b.map(f32::from),
        [0, 1, 2].map(|i| (2. * a[i] as f32 + b[i] as f32) / 3.), [0, 1, 2].map(|i| (a[i] as f32 + 2. * b[i] as f32) / 3.)];
    let mut indices = 0u32;
    for (n, p) in pixels.iter().enumerate() {
        let best = (0..4).min_by(|&x, &y| {
            let d = |k: usize| (0..3).map(|i| (palette[k][i] - p[i] as f32).powi(2)).sum::<f32>();
            d(x).total_cmp(&d(y))
        }).unwrap();
        indices |= (best as u32) << (2 * n);
    }
    out[4..8].copy_from_slice(&indices.to_le_bytes());
    out
}

fn encode_alpha(pixels: &[[u8; 4]; 16]) -> [u8; 8] {
    let (lo, hi) = pixels.iter().fold((255u8, 0u8), |(a, b), p| (a.min(p[3]), b.max(p[3])));
    let mut out = [0u8; 8];
    out[0] = hi;
    out[1] = lo;
    if hi == lo { return out; }
    let palette: Vec<f32> = (0..8).map(|i| match i { 0 => hi as f32, 1 => lo as f32, k => ((8 - k) as f32 * hi as f32 + (k - 1) as f32 * lo as f32) / 7. }).collect();
    let mut bits = 0u64;
    for (n, p) in pixels.iter().enumerate() {
        let best = (0..8).min_by(|&x, &y| (palette[x] - p[3] as f32).abs().total_cmp(&(palette[y] - p[3] as f32).abs())).unwrap();
        bits |= (best as u64) << (3 * n);
    }
    out[2..8].copy_from_slice(&bits.to_le_bytes()[..6]);
    out
}

/// BC1 ("DXT1") or BC3 ("DXT5") data for an image whose sides are multiples of 4.
pub fn encode(img: &RgbaImage, format: &str) -> Vec<u8> {
    let mut out = Vec::new();
    for by in 0..img.height().div_ceil(4) { for bx in 0..img.width().div_ceil(4) {
        let pixels = block_pixels(img, bx, by);
        if format == "DXT5" { out.extend(encode_alpha(&pixels)); }
        out.extend(encode_color(&pixels));
    } }
    out
}

pub fn decode(data: &[u8], width: u32, height: u32, format: &str) -> R<RgbaImage> {
    let size = if format == "DXT5" { 16 } else { 8 };
    let (bw, bh) = (width.div_ceil(4), height.div_ceil(4));
    if data.len() < (bw * bh) as usize * size { return Err("Datos DXT incompletos.".into()); }
    let mut img = RgbaImage::new(width, height);
    for by in 0..bh { for bx in 0..bw {
        let block = &data[((by * bw + bx) as usize) * size..][..size];
        let (alpha, color) = if size == 16 { (Some(&block[..8]), &block[8..]) } else { (None, block) };
        let (c0, c1) = (u16::from_le_bytes([color[0], color[1]]), u16::from_le_bytes([color[2], color[3]]));
        let (a, b) = (from565(c0), from565(c1));
        let mix = |wa: u32, wb: u32, d: u32| [0, 1, 2].map(|i| ((a[i] as u32 * wa + b[i] as u32 * wb) / d) as u8);
        let palette: [[u8; 4]; 4] = if c0 > c1 || alpha.is_some() {
            let (p2, p3) = (mix(2, 1, 3), mix(1, 2, 3));
            [[a[0], a[1], a[2], 255], [b[0], b[1], b[2], 255], [p2[0], p2[1], p2[2], 255], [p3[0], p3[1], p3[2], 255]]
        } else {
            let p2 = mix(1, 1, 2);
            [[a[0], a[1], a[2], 255], [b[0], b[1], b[2], 255], [p2[0], p2[1], p2[2], 255], [0, 0, 0, 0]]
        };
        let indices = u32::from_le_bytes(color[4..8].try_into().unwrap());
        let alphas: Option<(Vec<u8>, u64)> = alpha.map(|al| {
            let (a0, a1) = (al[0] as u32, al[1] as u32);
            let table: Vec<u8> = (0..8).map(|k| match k {
                0 => a0 as u8, 1 => a1 as u8,
                k if a0 > a1 => (((8 - k) * a0 + (k - 1) * a1) / 7) as u8,
                6 => 0, 7 => 255,
                k => (((6 - k) * a0 + (k - 1) * a1) / 5) as u8,
            }).collect();
            let mut bits = [0u8; 8];
            bits[..6].copy_from_slice(&al[2..8]);
            (table, u64::from_le_bytes(bits))
        });
        for n in 0..16u32 {
            let (x, y) = (bx * 4 + n % 4, by * 4 + n / 4);
            if x >= width || y >= height { continue; }
            let mut px = palette[((indices >> (2 * n)) & 3) as usize];
            if let Some((table, bits)) = &alphas { px[3] = table[((bits >> (3 * n)) & 7) as usize]; }
            img.put_pixel(x, y, image::Rgba(px));
        }
    } }
    Ok(img)
}

/// A picture from PNG/JPEG/WebP bytes or an uncompressed/DXT DDS file.
pub fn load(raw: &[u8]) -> R<RgbaImage> {
    if raw.starts_with(b"DDS ") && raw.len() >= 128 {
        let u = |o: usize| u32::from_le_bytes(raw[o..o + 4].try_into().unwrap());
        let (height, width, fourcc) = (u(12), u(16), &raw[84..88]);
        if width == 0 || height == 0 || width as u64 * height as u64 > 4096 * 4096 { return Err("Imagen DDS no válida.".into()); }
        return match fourcc {
            b"DXT1" => decode(&raw[128..], width, height, "DXT1"),
            b"DXT5" => decode(&raw[128..], width, height, "DXT5"),
            _ if u(88) == 32 => {
                // Uncompressed 32-bit BGRA.
                let pixels = raw.get(128..128 + (width * height * 4) as usize).ok_or("DDS incompleto.")?;
                Ok(RgbaImage::from_raw(width, height, pixels.chunks_exact(4).flat_map(|p| [p[2], p[1], p[0], p[3]]).collect()).ok_or("DDS incompleto.")?)
            }
            _ => Err("Formato DDS no compatible.".into()),
        };
    }
    Ok(image::load_from_memory(raw).map_err(err)?.to_rgba8())
}

/// Lanczos resize on premultiplied alpha, as PIL resizes RGBA images.
pub fn resize(img: &RgbaImage, width: u32, height: u32) -> RgbaImage {
    if img.dimensions() == (width, height) { return img.clone(); }
    let premultiplied = image::ImageBuffer::<image::Rgba<f32>, Vec<f32>>::from_fn(img.width(), img.height(), |x, y| {
        let p = img.get_pixel(x, y).0;
        // image's f32 pixels live in 0..1 (resize clamps to that range).
        let a = p[3] as f32 / 255.;
        image::Rgba([p[0] as f32 / 255. * a, p[1] as f32 / 255. * a, p[2] as f32 / 255. * a, a])
    });
    let scaled = image::imageops::resize(&premultiplied, width, height, FilterType::Lanczos3);
    RgbaImage::from_fn(width, height, |x, y| {
        let p = scaled.get_pixel(x, y).0;
        let a = p[3].clamp(0., 1.);
        let un = |v: f32| if a > 0. { (v / a * 255.).round().clamp(0., 255.) as u8 } else { 0 };
        image::Rgba([un(p[0]), un(p[1]), un(p[2]), (a * 255.).round() as u8])
    })
}

/// ImageOps.fit: centre crop to the target proportions, then resize.
pub fn fit(img: &RgbaImage, width: u32, height: u32) -> RgbaImage {
    let (w, h) = (img.width() as f64, img.height() as f64);
    let out_ratio = width as f64 / height as f64;
    let (cw, ch) = if (w / h - out_ratio).abs() < 1e-9 { (w, h) } else if w / h > out_ratio { (out_ratio * h, h) } else { (w, w / out_ratio) };
    let (left, top) = (((w - cw) * 0.5).round() as u32, ((h - ch) * 0.5).round() as u32);
    let cropped = image::imageops::crop_imm(img, left, top, (cw.round() as u32).max(1), (ch.round() as u32).max(1)).to_image();
    resize(&cropped, width, height)
}

pub fn png(img: &RgbaImage) -> R<Vec<u8>> {
    let mut out = std::io::Cursor::new(Vec::new());
    img.write_to(&mut out, image::ImageFormat::Png).map_err(err)?;
    Ok(out.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn dxt_roundtrip_is_close() {
        let img = RgbaImage::from_fn(16, 16, |x, y| image::Rgba([(x * 16) as u8, (y * 16) as u8, 128, if x < 8 { 255 } else { 40 }]));
        for format in ["DXT1", "DXT5"] {
            let data = encode(&img, format);
            assert_eq!(data.len(), if format == "DXT5" { 256 } else { 128 });
            let back = decode(&data, 16, 16, format).unwrap();
            let error: f64 = img.pixels().zip(back.pixels()).map(|(a, b)| (0..3).map(|i| (a[i] as f64 - b[i] as f64).abs()).sum::<f64>()).sum::<f64>() / (16. * 16. * 3.);
            // A 2-D gradient inside a 4x4 block is lossy in BC1 by design.
            assert!(error < 12., "{format} error medio {error}");
            if format == "DXT5" { assert!(back.pixels().zip(img.pixels()).all(|(b, a)| (b[3] as i32 - a[3] as i32).abs() <= 20)); }
        }
    }
    #[test]
    fn fit_crops_to_proportions() {
        let img = RgbaImage::from_pixel(400, 100, image::Rgba([10, 20, 30, 255]));
        assert_eq!(fit(&img, 100, 100).dimensions(), (100, 100));
        assert_eq!(resize(&img, 50, 25).get_pixel(10, 10).0, [10, 20, 30, 255]);
    }
}
