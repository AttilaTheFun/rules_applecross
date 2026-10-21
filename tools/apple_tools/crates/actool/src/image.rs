//! Pixels: decoding source PNGs, resampling, and the CgBI PNGs iOS bundles
//! carry for their icons.

/// 8-bit RGBA with straight (unpremultiplied) alpha, row-major.
#[derive(Clone)]
pub struct Rgba {
    pub width: usize,
    pub height: usize,
    pub data: Vec<u8>,
}

impl Rgba {
    /// Premultiplied BGRA, the byte order CoreUI's ARGB renditions and CgBI
    /// PNGs both store.
    pub fn premultiplied_bgra(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.data.len());
        for p in self.data.chunks_exact(4) {
            let a = p[3] as u32;
            let m = |c: u8| ((c as u32 * a + 127) / 255) as u8;
            out.extend_from_slice(&[m(p[2]), m(p[1]), m(p[0]), p[3]]);
        }
        out
    }
}

pub fn decode_png(bytes: &[u8]) -> Result<Rgba, String> {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().map_err(|e| e.to_string())?;
    let mut buf = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).map_err(|e| e.to_string())?;
    let (w, h) = (info.width as usize, info.height as usize);
    let src = &buf[..info.buffer_size()];
    let mut data = Vec::with_capacity(w * h * 4);
    match info.color_type {
        png::ColorType::Rgba => data.extend_from_slice(src),
        png::ColorType::Rgb => src.chunks_exact(3).for_each(|p| data.extend_from_slice(&[p[0], p[1], p[2], 255])),
        png::ColorType::GrayscaleAlpha => src.chunks_exact(2).for_each(|p| data.extend_from_slice(&[p[0], p[0], p[0], p[1]])),
        png::ColorType::Grayscale => src.iter().for_each(|&g| data.extend_from_slice(&[g, g, g, 255])),
        png::ColorType::Indexed => return Err("indexed PNG left unexpanded".into()),
    }
    Ok(Rgba { width: w, height: h, data })
}

/// Premultiplied BGRA pixels, row-major.
#[derive(Clone)]
pub struct Bgra {
    pub width: usize,
    pub height: usize,
    pub data: Vec<u8>,
}

impl Rgba {
    pub fn to_bgra(&self) -> Bgra {
        Bgra { width: self.width, height: self.height, data: self.premultiplied_bgra() }
    }
}

/// Resampling as Xcode's actool does it, by drawing with CoreGraphics' high
/// interpolation quality (docs/clean-room/actool.md): separable, horizontal
/// pass first, on premultiplied pixels. Output pixel i samples the source at
/// c = i * (S - 1) / (D - 1) through a Lanczos-2 kernel stretched by the same
/// factor, except that the tap nearest c weighs 1; taps beyond an edge count
/// for the edge pixel; the weights are normalized, stored as 14-bit fixed
/// point with the rounding residual on the nearest tap; each pass rounds half
/// up and clamps to 8 bits.
pub fn resize(src: &Bgra, dw: usize, dh: usize) -> Bgra {
    let wx = weights(src.width, dw);
    let wy = weights(src.height, dh);
    let mut mid = vec![0u8; dw * src.height * 4];
    for y in 0..src.height {
        let row = &src.data[y * src.width * 4..(y + 1) * src.width * 4];
        for (x, taps) in wx.iter().enumerate() {
            for c in 0..4 {
                let acc: i64 = taps.iter().map(|&(j, w)| row[j * 4 + c] as i64 * w as i64).sum();
                mid[(y * dw + x) * 4 + c] = round14(acc);
            }
        }
    }
    let mut out = vec![0u8; dw * dh * 4];
    for (y, taps) in wy.iter().enumerate() {
        for x in 0..dw {
            for c in 0..4 {
                let acc: i64 = taps.iter().map(|&(j, w)| mid[(j * dw + x) * 4 + c] as i64 * w as i64).sum();
                out[(y * dw + x) * 4 + c] = round14(acc);
            }
        }
    }
    Bgra { width: dw, height: dh, data: out }
}

fn round14(acc: i64) -> u8 {
    ((acc + 8192) >> 14).clamp(0, 255) as u8
}

fn lanczos2(x: f64) -> f64 {
    if x.abs() >= 2.0 {
        return 0.0;
    }
    if x == 0.0 {
        return 1.0;
    }
    let a = std::f64::consts::PI * x;
    (a.sin() / a) * ((a / 2.0).sin() / (a / 2.0))
}

/// Per output pixel, (source index, 14-bit weight) pairs.
fn weights(src: usize, dst: usize) -> Vec<Vec<(usize, i32)>> {
    if src == dst {
        return (0..dst).map(|i| vec![(i, 16384)]).collect();
    }
    let s = if dst > 1 { (src - 1) as f64 / (dst - 1) as f64 } else { 1.0 };
    let last = src as i64 - 1;
    (0..dst)
        .map(|i| {
            let c = i as f64 * s;
            let nearest = (c + 0.5).floor() as i64;
            let mut taps: Vec<(usize, f64)> = Vec::new();
            let lo = (c - 2.0 * s).floor() as i64;
            let hi = (c + 2.0 * s).ceil() as i64;
            for j in lo..=hi {
                let w = if j == nearest { 1.0 } else { lanczos2((j as f64 - c) / s) };
                if w == 0.0 {
                    continue;
                }
                let jj = j.clamp(0, last) as usize;
                match taps.iter_mut().find(|t| t.0 == jj) {
                    Some(t) => t.1 += w,
                    None => taps.push((jj, w)),
                }
            }
            let total: f64 = taps.iter().map(|t| t.1).sum();
            let mut q: Vec<(usize, i32)> = taps.iter().map(|&(j, w)| (j, (w / total * 16384.0).round() as i32)).collect();
            let sum: i32 = q.iter().map(|t| t.1).sum();
            let near = nearest.clamp(0, last) as usize;
            if let Some(t) = q.iter_mut().find(|t| t.0 == near) {
                t.1 += 16384 - sum;
            }
            q
        })
        .collect()
}

// ---------------------------------------------------------------- CgBI PNG

extern "C" {
    fn deflate_parts(parts: *const *const u8, sizes: *const usize, n: std::ffi::c_int, out: *mut u8, out_cap: usize, out_sizes: *mut usize) -> std::ffi::c_long;
}

// Linked so the shim resolves zlib from the same static build.
use libz_sys as _;

/// One raw-deflate stream over `parts`, a full flush after each but the last
/// (which finishes the stream); returns the bytes of each part. zlib level 9,
/// a 32 KiB window, memLevel 9 and Z_FILTERED, as Xcode's icons carry.
fn deflate_split(parts: &[&[u8]]) -> Vec<Vec<u8>> {
    let ptrs: Vec<*const u8> = parts.iter().map(|p| p.as_ptr()).collect();
    let sizes: Vec<usize> = parts.iter().map(|p| p.len()).collect();
    let cap = parts.iter().map(|p| p.len() + p.len() / 1000 + 64).sum::<usize>() + 64;
    let mut out = vec![0u8; cap];
    let mut out_sizes = vec![0usize; parts.len()];
    let n = unsafe { deflate_parts(ptrs.as_ptr(), sizes.as_ptr(), parts.len() as std::ffi::c_int, out.as_mut_ptr(), cap, out_sizes.as_mut_ptr()) };
    assert!(n >= 0, "deflate failed");
    let mut res = Vec::new();
    let mut o = 0;
    for s in out_sizes {
        res.push(out[o..o + s].to_vec());
        o += s;
    }
    res
}

fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    let start = out.len();
    out.extend_from_slice(kind);
    out.extend_from_slice(data);
    let crc = crc32(&out[start..]);
    out.extend_from_slice(&crc.to_be_bytes());
}

fn crc32(data: &[u8]) -> u32 {
    let mut c = 0xffff_ffffu32;
    for &b in data {
        c ^= b as u32;
        for _ in 0..8 {
            c = if c & 1 != 0 { (c >> 1) ^ 0xedb8_8320 } else { c >> 1 };
        }
    }
    !c
}

/// An Apple-optimized ("CgBI") PNG: premultiplied BGRA, unfiltered rows,
/// raw deflate split into two IDAT chunks at the iDOT boundary, and the
/// colour and EXIF chunks ImageIO writes.
pub fn cgbi_png(img: &Bgra) -> Vec<u8> {
    let (w, h) = (img.width as u32, img.height as u32);
    let bgra = &img.data;
    let stride = img.width * 4;
    let first_rows = img.height / 2 + img.height % 2;
    let mut raw = Vec::with_capacity((stride + 1) * img.height);
    for y in 0..img.height {
        raw.push(0);
        raw.extend_from_slice(&bgra[y * stride..(y + 1) * stride]);
    }
    let split = first_rows * (stride + 1);
    let parts = deflate_split(&[&raw[..split], &raw[split..]]);

    let mut out = b"\x89PNG\r\n\x1a\n".to_vec();
    chunk(&mut out, b"CgBI", &0x5000_2006u32.to_be_bytes());
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&w.to_be_bytes());
    ihdr.extend_from_slice(&h.to_be_bytes());
    ihdr.extend_from_slice(&[8, 6, 0, 0, 0]);
    chunk(&mut out, b"IHDR", &ihdr);
    chunk(&mut out, b"gAMA", &45455u32.to_be_bytes());
    chunk(&mut out, b"sRGB", &[0]);
    let mut chrm = Vec::new();
    for v in [31270u32, 32900, 64000, 33000, 30000, 60000, 15000, 6000] {
        chrm.extend_from_slice(&v.to_be_bytes());
    }
    chunk(&mut out, b"cHRM", &chrm);
    chunk(&mut out, b"eXIf", &exif(w, h));
    // iDOT: two segments; the first's row count and IDAT offset (from the
    // iDOT chunk's start), then the second's.
    let idot_start = out.len();
    let first_idat_offset = 12 + 28; // iDOT chunk length
    let second_idat_offset = first_idat_offset + 12 + parts[0].len();
    let mut idot = Vec::new();
    for v in [2u32, 0, first_rows as u32, first_idat_offset as u32, (img.height - first_rows) as u32, first_rows as u32, second_idat_offset as u32] {
        idot.extend_from_slice(&v.to_be_bytes());
    }
    chunk(&mut out, b"iDOT", &idot);
    debug_assert_eq!(out.len() - idot_start, first_idat_offset);
    chunk(&mut out, b"IDAT", &parts[0]);
    chunk(&mut out, b"IDAT", &parts[1]);
    chunk(&mut out, b"IEND", &[]);
    out
}

/// Big-endian TIFF with one Exif IFD: ColorSpace 1 (sRGB) and the pixel
/// dimensions.
fn exif(w: u32, h: u32) -> Vec<u8> {
    let mut e = Vec::new();
    e.extend_from_slice(b"MM\x00\x2a");
    e.extend_from_slice(&8u32.to_be_bytes());
    // IFD0: one entry, ExifIFDPointer -> 26.
    e.extend_from_slice(&1u16.to_be_bytes());
    e.extend_from_slice(&[0x87, 0x69, 0x00, 0x04]);
    e.extend_from_slice(&1u32.to_be_bytes());
    e.extend_from_slice(&26u32.to_be_bytes());
    e.extend_from_slice(&0u32.to_be_bytes());
    // Exif IFD: three entries.
    e.extend_from_slice(&3u16.to_be_bytes());
    e.extend_from_slice(&[0xa0, 0x01, 0x00, 0x03]);
    e.extend_from_slice(&1u32.to_be_bytes());
    e.extend_from_slice(&[0x00, 0x01, 0x00, 0x00]);
    e.extend_from_slice(&[0xa0, 0x02, 0x00, 0x04]);
    e.extend_from_slice(&1u32.to_be_bytes());
    e.extend_from_slice(&w.to_be_bytes());
    e.extend_from_slice(&[0xa0, 0x03, 0x00, 0x04]);
    e.extend_from_slice(&1u32.to_be_bytes());
    e.extend_from_slice(&h.to_be_bytes());
    e.extend_from_slice(&0u32.to_be_bytes());
    e
}
