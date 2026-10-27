//! A very small PDF writer: pages, filled/stroked rectangles and text in the
//! standard Helvetica faces. No compression, no external crates.

use std::fs;
use std::io;
use std::path::Path;

use crate::fonts;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
}

impl Color {
    pub const BLACK: Color = Color { r: 0.0, g: 0.0, b: 0.0 };
    pub const WHITE: Color = Color { r: 1.0, g: 1.0, b: 1.0 };

    pub const fn rgb8(r: u8, g: u8, b: u8) -> Color {
        Color { r: r as f32 / 255.0, g: g as f32 / 255.0, b: b as f32 / 255.0 }
    }

    /// Parse RRGGBB or AARRGGBB (the leading '#' and the alpha channel are
    /// ignored, like the app's colour picker values).
    pub fn from_hex(s: &str) -> Option<Color> {
        let s = s.trim().trim_start_matches('#');
        let s = if s.len() == 8 { &s[2..] } else { s };
        if s.len() != 6 || !s.chars().all(|c| c.is_ascii_hexdigit()) {
            return None;
        }
        let v = u32::from_str_radix(s, 16).ok()?;
        Some(Color::rgb8((v >> 16) as u8, (v >> 8) as u8, v as u8))
    }

    /// Composite self over bg with alpha in 0..=1. PDF opacity is avoided by
    /// pre-blending, which gives the same visual result on a solid page.
    pub fn blend(self, bg: Color, alpha: f32) -> Color {
        let a = alpha.clamp(0.0, 1.0);
        Color {
            r: self.r * a + bg.r * (1.0 - a),
            g: self.g * a + bg.g * (1.0 - a),
            b: self.b * a + bg.b * (1.0 - a),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Align {
    Left,
    Center,
    Right,
}

pub struct Image {
    pub name: String,
    pub width: u32,
    pub height: u32,
    /// 8-bit RGB, row major, no padding.
    pub rgb: Vec<u8>,
}

struct Page {
    w: f32,
    h: f32,
    ops: String,
}

pub struct Pdf {
    pages: Vec<Page>,
    images: Vec<Image>,
}

fn n(v: f32) -> String {
    let mut s = format!("{:.2}", v);
    while s.contains('.') && s.ends_with('0') {
        s.pop();
    }
    if s.ends_with('.') {
        s.pop();
    }
    s
}

/// Encode a string to WinAnsi, the encoding declared for both fonts.
fn winansi(s: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(s.len());
    for ch in s.chars() {
        let c = ch as u32;
        let b = if c < 0x100 {
            c as u8
        } else {
            match c {
                0x2018 => 0x91,
                0x2019 => 0x92,
                0x201c => 0x93,
                0x201d => 0x94,
                0x2022 => 0x95,
                0x2013 => 0x96,
                0x2014 => 0x97,
                0x2026 => 0x85,
                0x2122 => 0x99,
                _ => b'?',
            }
        };
        out.push(b);
    }
    out
}

fn escape(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() + 8);
    for &b in bytes {
        match b {
            b'(' | b')' | b'\\' => {
                s.push('\\');
                s.push(b as char);
            }
            32..=126 => s.push(b as char),
            _ => s.push_str(&format!("\\{:03o}", b)),
        }
    }
    s
}

impl Pdf {
    pub fn new() -> Self {
        Pdf { pages: Vec::new(), images: Vec::new() }
    }

    pub fn add_image(&mut self, name: &str, width: u32, height: u32, rgb: Vec<u8>) -> usize {
        self.images.push(Image { name: name.to_string(), width, height, rgb });
        self.images.len() - 1
    }

    pub fn page(&mut self, w: f32, h: f32) {
        self.pages.push(Page { w, h, ops: String::new() });
    }

    fn flip(&self, y: f32) -> f32 {
        self.pages.last().map(|p| p.h - y).unwrap_or(y)
    }

    pub fn fill_rect(&mut self, x: f32, y: f32, w: f32, h: f32, color: Color) {
        let fy = self.flip(y + h);
        let op = format!(
            "{} {} {} rg {} {} {} {} re f\n",
            n(color.r), n(color.g), n(color.b), n(x), n(fy), n(w), n(h)
        );
        if let Some(p) = self.pages.last_mut() {
            p.ops.push_str(&op);
        }
    }

    pub fn stroke_rect(&mut self, x: f32, y: f32, w: f32, h: f32, color: Color, lw: f32) {
        let fy = self.flip(y + h);
        let op = format!(
            "{} {} {} RG {} w {} {} {} {} re S\n",
            n(color.r), n(color.g), n(color.b), n(lw), n(x), n(fy), n(w), n(h)
        );
        if let Some(p) = self.pages.last_mut() {
            p.ops.push_str(&op);
        }
    }

    pub fn text(
        &mut self,
        s: &str,
        x: f32,
        baseline: f32,
        size: f32,
        color: Color,
        bold: bool,
        align: Align,
    ) {
        if s.is_empty() {
            return;
        }
        let width = fonts::text_width(s, size, bold);
        let x = match align {
            Align::Left => x,
            Align::Center => x - width / 2.0,
            Align::Right => x - width,
        };
        let font = if bold { "F2" } else { "F1" };
        let op = format!(
            "BT /{} {} Tf {} {} {} rg 1 0 0 1 {} {} Tm ({}) Tj ET\n",
            font,
            n(size),
            n(color.r),
            n(color.g),
            n(color.b),
            n(x),
            n(self.flip(baseline)),
            escape(&winansi(s))
        );
        if let Some(p) = self.pages.last_mut() {
            p.ops.push_str(&op);
        }
    }

    pub fn image(&mut self, index: usize, x: f32, y: f32, w: f32, h: f32) {
        let name = match self.images.get(index) {
            Some(i) => i.name.clone(),
            None => return,
        };
        let fy = self.flip(y + h);
        let op = format!("q {} 0 0 {} {} {} cm /{} Do Q\n", n(w), n(h), n(x), n(fy), name);
        if let Some(p) = self.pages.last_mut() {
            p.ops.push_str(&op);
        }
    }

    /// Serialise the whole document.
    pub fn bytes(&self, title: &str) -> Vec<u8> {
        let mut objs: Vec<Vec<u8>> = Vec::new();
        let npages = self.pages.len();
        let nimg = self.images.len();
        // ids: 1 catalog, 2 pages, 3 F1, 4 F2, 5.. images, pages, streams, info
        let first_img = 5;
        let first_page = first_img + nimg;
        let first_stream = first_page + npages;
        let info_id = first_stream + npages;

        objs.push(b"<< /Type /Catalog /Pages 2 0 R >>".to_vec());
        objs.push(
            format!(
                "<< /Type /Pages /Count {} /Kids [{}] >>",
                npages,
                (0..npages)
                    .map(|i| format!("{} 0 R", first_page + i))
                    .collect::<Vec<_>>()
                    .join(" ")
            )
            .into_bytes(),
        );
        objs.push(
            b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>"
                .to_vec(),
        );
        objs.push(
            b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica-Bold /Encoding /WinAnsiEncoding >>"
                .to_vec(),
        );

        for img in &self.images {
            let mut o = format!(
                "<< /Type /XObject /Subtype /Image /Width {} /Height {} /ColorSpace /DeviceRGB /BitsPerComponent 8 /Length {} >>\nstream\n",
                img.width,
                img.height,
                img.rgb.len()
            )
            .into_bytes();
            o.extend_from_slice(&img.rgb);
            o.extend_from_slice(b"\nendstream");
            objs.push(o);
        }

        let xobj = if nimg == 0 {
            String::new()
        } else {
            format!(
                " /XObject << {} >>",
                self.images
                    .iter()
                    .enumerate()
                    .map(|(i, im)| format!("/{} {} 0 R", im.name, first_img + i))
                    .collect::<Vec<_>>()
                    .join(" ")
            )
        };

        for (i, p) in self.pages.iter().enumerate() {
            objs.push(
                format!(
                    "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {} {}] /Resources << /Font << /F1 3 0 R /F2 4 0 R >>{} >> /Contents {} 0 R >>",
                    n(p.w),
                    n(p.h),
                    xobj,
                    first_stream + i
                )
                .into_bytes(),
            );
        }
        for p in &self.pages {
            let body = p.ops.as_bytes();
            let mut o = format!("<< /Length {} >>\nstream\n", body.len()).into_bytes();
            o.extend_from_slice(body);
            o.extend_from_slice(b"endstream");
            objs.push(o);
        }
        objs.push(
            format!(
                "<< /Title ({}) /Producer (songbook-rs) /CreationDate (D:20240101000000Z) >>",
                escape(&winansi(title))
            )
            .into_bytes(),
        );

        let mut out: Vec<u8> = Vec::new();
        out.extend_from_slice(b"%PDF-1.4\n%\xe2\xe3\xcf\xd3\n");
        let mut offsets = Vec::with_capacity(objs.len());
        for (i, obj) in objs.iter().enumerate() {
            offsets.push(out.len());
            out.extend_from_slice(format!("{} 0 obj\n", i + 1).as_bytes());
            out.extend_from_slice(obj);
            out.extend_from_slice(b"\nendobj\n");
        }
        let xref = out.len();
        out.extend_from_slice(
            format!("xref\n0 {}\n0000000000 65535 f \n", objs.len() + 1).as_bytes(),
        );
        for off in &offsets {
            out.extend_from_slice(format!("{:010} 00000 n \n", off).as_bytes());
        }
        out.extend_from_slice(
            format!(
                "trailer\n<< /Size {} /Root 1 0 R /Info {} 0 R >>\nstartxref\n{}\n%%EOF\n",
                objs.len() + 1,
                info_id,
                xref
            )
            .as_bytes(),
        );
        out
    }

    pub fn save(&self, path: &Path, title: &str) -> io::Result<()> {
        fs::write(path, self.bytes(title))
    }
}

impl Default for Pdf {
    fn default() -> Self {
        Pdf::new()
    }
}

// --------------------------------------------------------------------------
// zlib inflate + PNG decoding (only used for the optional logo)
// --------------------------------------------------------------------------

struct BitReader<'a> {
    data: &'a [u8],
    pos: usize,
    buf: u64,
    cnt: u32,
}

impl<'a> BitReader<'a> {
    fn new(data: &'a [u8]) -> Self {
        BitReader { data, pos: 0, buf: 0, cnt: 0 }
    }

    /// Drop buffered bits up to the next byte boundary.
    fn align(&mut self) {
        let rest = self.cnt % 8;
        self.buf >>= rest;
        self.cnt -= rest;
    }

    fn bits(&mut self, need: u32) -> Result<u32, String> {
        while self.cnt < need {
            let b = *self.data.get(self.pos).ok_or("truncated deflate stream")?;
            self.pos += 1;
            self.buf |= (b as u64) << self.cnt;
            self.cnt += 8;
        }
        let v = (self.buf & ((1u64 << need) - 1)) as u32;
        self.buf >>= need;
        self.cnt -= need;
        Ok(v)
    }
}

struct Huff {
    count: [u16; 16],
    symbol: Vec<u16>,
}

impl Huff {
    fn new(lengths: &[u8]) -> Huff {
        let mut count = [0u16; 16];
        for &l in lengths {
            count[l as usize] += 1;
        }
        count[0] = 0;
        let mut offs = [0u16; 16];
        for i in 1..16 {
            offs[i] = offs[i - 1] + count[i - 1];
        }
        let mut symbol = vec![0u16; lengths.len()];
        for (sym, &l) in lengths.iter().enumerate() {
            if l != 0 {
                symbol[offs[l as usize] as usize] = sym as u16;
                offs[l as usize] += 1;
            }
        }
        Huff { count, symbol }
    }

    fn decode(&mut self, br: &mut BitReader) -> Result<u16, String> {
        let mut code: i32 = 0;
        let mut first: i32 = 0;
        let mut index: i32 = 0;
        for len in 1..16 {
            code |= br.bits(1)? as i32;
            let count = self.count[len] as i32;
            if code - count < first {
                return Ok(self.symbol[(index + (code - first)) as usize]);
            }
            index += count;
            first = (first + count) << 1;
            code <<= 1;
        }
        Err("invalid huffman code".into())
    }
}

const LEN_BASE: [u16; 29] = [
    3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115,
    131, 163, 195, 227, 258,
];
const LEN_EXTRA: [u8; 29] = [
    0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0,
];
const DIST_BASE: [u16; 30] = [
    1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537,
    2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577,
];
const DIST_EXTRA: [u8; 30] = [
    0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12,
    13, 13,
];

fn inflate_codes(
    br: &mut BitReader,
    out: &mut Vec<u8>,
    lit: &mut Huff,
    dist: &mut Huff,
) -> Result<(), String> {
    loop {
        let sym = lit.decode(br)?;
        match sym {
            0..=255 => out.push(sym as u8),
            256 => return Ok(()),
            257..=285 => {
                let i = (sym - 257) as usize;
                let len = LEN_BASE[i] as usize + br.bits(LEN_EXTRA[i] as u32)? as usize;
                let dsym = dist.decode(br)? as usize;
                if dsym >= 30 {
                    return Err("invalid distance symbol".into());
                }
                let d = DIST_BASE[dsym] as usize + br.bits(DIST_EXTRA[dsym] as u32)? as usize;
                if d > out.len() {
                    return Err("distance too far back".into());
                }
                let start = out.len() - d;
                for k in 0..len {
                    let b = out[start + k];
                    out.push(b);
                }
            }
            _ => return Err("invalid literal/length symbol".into()),
        }
    }
}

/// Minimal zlib/deflate decompressor (RFC 1950 / RFC 1951).
pub fn inflate(data: &[u8]) -> Result<Vec<u8>, String> {
    if data.len() >= 2 {
        let cmf = data[0];
        let flg = data[1];
        if cmf & 0x0f != 8 {
            return Err("not a zlib/deflate stream".into());
        }
        if ((cmf as u16) << 8 | flg as u16) % 31 != 0 {
            return Err("bad zlib header".into());
        }
    }
    let mut br = BitReader::new(if data.len() > 2 { &data[2..] } else { data });
    let mut out: Vec<u8> = Vec::with_capacity(data.len() * 4);

    loop {
        let last = br.bits(1)?;
        let btype = br.bits(2)?;
        match btype {
            0 => {
                br.align();
                let len = br.bits(16)? as usize;
                let _nlen = br.bits(16)?;
                for _ in 0..len {
                    out.push(br.bits(8)? as u8);
                }
            }
            1 => {
                let mut lengths = [0u8; 288];
                for (i, l) in lengths.iter_mut().enumerate() {
                    *l = if i < 144 {
                        8
                    } else if i < 256 {
                        9
                    } else if i < 280 {
                        7
                    } else {
                        8
                    };
                }
                let mut lit = Huff::new(&lengths);
                let mut dist = Huff::new(&[5u8; 32]);
                inflate_codes(&mut br, &mut out, &mut lit, &mut dist)?;
            }
            2 => {
                let hlit = br.bits(5)? as usize + 257;
                let hdist = br.bits(5)? as usize + 1;
                let hclen = br.bits(4)? as usize + 4;
                const ORDER: [usize; 19] = [
                    16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15,
                ];
                let mut cl = [0u8; 19];
                for i in 0..hclen {
                    cl[ORDER[i]] = br.bits(3)? as u8;
                }
                let mut clh = Huff::new(&cl);
                let mut lengths = vec![0u8; hlit + hdist];
                let mut i = 0;
                while i < hlit + hdist {
                    let sym = clh.decode(&mut br)?;
                    match sym {
                        0..=15 => {
                            lengths[i] = sym as u8;
                            i += 1;
                        }
                        16 => {
                            if i == 0 {
                                return Err("repeat with no previous length".into());
                            }
                            let prev = lengths[i - 1];
                            let reps = 3 + br.bits(2)? as usize;
                            for _ in 0..reps {
                                if i < lengths.len() {
                                    lengths[i] = prev;
                                    i += 1;
                                }
                            }
                        }
                        17 => {
                            let reps = 3 + br.bits(3)? as usize;
                            i = (i + reps).min(lengths.len());
                        }
                        18 => {
                            let reps = 11 + br.bits(7)? as usize;
                            i = (i + reps).min(lengths.len());
                        }
                        _ => return Err("invalid code length symbol".into()),
                    }
                }
                let mut lit = Huff::new(&lengths[..hlit]);
                let mut dist = Huff::new(&lengths[hlit..]);
                inflate_codes(&mut br, &mut out, &mut lit, &mut dist)?;
            }
            _ => return Err("invalid deflate block type".into()),
        }
        if last == 1 {
            break;
        }
    }
    Ok(out)
}

/// Decode an 8-bit, non-interlaced PNG into 8-bit RGB, compositing alpha over bg.
pub fn load_png(data: &[u8], bg: Color) -> Result<(u32, u32, Vec<u8>), String> {
    if data.len() < 8 || &data[..8] != b"\x89PNG\r\n\x1a\n" {
        return Err("not a PNG file".to_string());
    }
    let mut pos = 8usize;
    let mut ihdr: Option<(u32, u32, u8, u8, u8)> = None;
    let mut palette: Vec<u8> = Vec::new();
    let mut idat: Vec<u8> = Vec::new();
    while pos + 8 <= data.len() {
        let len =
            u32::from_be_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]) as usize;
        let typ = &data[pos + 4..pos + 8];
        let body_start = pos + 8;
        let body_end = body_start.checked_add(len).ok_or("bad PNG chunk length")?;
        if body_end > data.len() {
            return Err("truncated PNG".into());
        }
        let body = &data[body_start..body_end];
        match typ {
            b"IHDR" if body.len() >= 13 => {
                ihdr = Some((
                    u32::from_be_bytes([body[0], body[1], body[2], body[3]]),
                    u32::from_be_bytes([body[4], body[5], body[6], body[7]]),
                    body[8],
                    body[9],
                    body[12],
                ));
            }
            b"PLTE" => palette.extend_from_slice(body),
            b"IDAT" => idat.extend_from_slice(body),
            b"IEND" => break,
            _ => {}
        }
        pos = body_end + 4;
    }
    let (width, height, depth, color_type, interlace) = ihdr.ok_or("PNG without IHDR")?;
    if depth != 8 {
        return Err("only 8-bit PNG images are supported".into());
    }
    if interlace != 0 {
        return Err("interlaced PNG images are not supported".into());
    }
    let channels: usize = match color_type {
        0 => 1,
        2 => 3,
        3 => 1,
        4 => 2,
        6 => 4,
        other => return Err(format!("unsupported PNG colour type {}", other)),
    };
    let raw = inflate(&idat)?;
    let stride = width as usize * channels;
    if raw.len() < (stride + 1) * height as usize {
        return Err("truncated PNG image data".into());
    }
    let mut pixels = vec![0u8; stride * height as usize];
    let mut prev = vec![0u8; stride];
    for y in 0..height as usize {
        let base = y * (stride + 1);
        let filter = raw[base];
        let mut line = raw[base + 1..base + 1 + stride].to_vec();
        match filter {
            0 => {}
            1 => {
                for i in channels..stride {
                    line[i] = line[i].wrapping_add(line[i - channels]);
                }
            }
            2 => {
                for i in 0..stride {
                    line[i] = line[i].wrapping_add(prev[i]);
                }
            }
            3 => {
                for i in 0..stride {
                    let a = if i >= channels { line[i - channels] as u16 } else { 0 };
                    let b = prev[i] as u16;
                    line[i] = line[i].wrapping_add(((a + b) / 2) as u8);
                }
            }
            4 => {
                for i in 0..stride {
                    let a = if i >= channels { line[i - channels] as i16 } else { 0 };
                    let b = prev[i] as i16;
                    let c = if i >= channels { prev[i - channels] as i16 } else { 0 };
                    let p = a + b - c;
                    let pa = (p - a).abs();
                    let pb = (p - b).abs();
                    let pc = (p - c).abs();
                    let pred = if pa <= pb && pa <= pc {
                        a
                    } else if pb <= pc {
                        b
                    } else {
                        c
                    };
                    line[i] = line[i].wrapping_add(pred as u8);
                }
            }
            other => return Err(format!("unsupported PNG filter {}", other)),
        }
        pixels[y * stride..(y + 1) * stride].copy_from_slice(&line);
        prev = line;
    }

    let mut rgb = vec![0u8; (width * height * 3) as usize];
    let bg8 = [
        (bg.r * 255.0).round() as u32,
        (bg.g * 255.0).round() as u32,
        (bg.b * 255.0).round() as u32,
    ];
    for i in 0..(width * height) as usize {
        let px = &pixels[i * channels..(i + 1) * channels];
        let (r, g, b, a) = match color_type {
            0 => (px[0], px[0], px[0], 255u32),
            2 => (px[0], px[1], px[2], 255),
            3 => {
                let idx = px[0] as usize * 3;
                if idx + 2 >= palette.len() {
                    return Err("PNG palette index out of range".into());
                }
                (palette[idx], palette[idx + 1], palette[idx + 2], 255)
            }
            4 => (px[0], px[0], px[0], px[1] as u32),
            _ => (px[0], px[1], px[2], px[3] as u32),
        };
        let mix = |c: u8, bgc: u32| -> u8 { ((c as u32 * a + bgc * (255 - a)) / 255) as u8 };
        rgb[i * 3] = mix(r, bg8[0]);
        rgb[i * 3 + 1] = mix(g, bg8[1]);
        rgb[i * 3 + 2] = mix(b, bg8[2]);
    }
    Ok((width, height, rgb))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_colors() {
        assert_eq!(Color::from_hex("#BDBDBD"), Some(Color::rgb8(0xBD, 0xBD, 0xBD)));
        assert_eq!(Color::from_hex("FF000000"), Some(Color::BLACK));
        assert_eq!(Color::from_hex("nope"), None);
    }

    #[test]
    fn blend_halfway() {
        let c = Color::BLACK.blend(Color::WHITE, 0.5);
        assert!((c.r - 0.5).abs() < 1e-6);
    }

    #[test]
    fn inflate_stored_block() {
        let data = [
            0x78, 0x01, 0x01, 0x05, 0x00, 0xfa, 0xff, b'h', b'e', b'l', b'l', b'o', 0x06, 0x2c,
            0x02, 0x15,
        ];
        assert_eq!(inflate(&data).unwrap(), b"hello");
    }

    #[test]
    fn inflate_fixed_huffman() {
        let data = [
            0x78, 0x01, 0x4b, 0x4c, 0x44, 0x05, 0x00, 0x33, 0x98, 0x06, 0x11,
        ];
        assert_eq!(inflate(&data).unwrap(), b"aaaaaaaaaaaaaaaa");
    }

    #[test]
    fn inflate_dynamic_huffman() {
        let text = b"The quick brown fox jumps over the lazy dog.".repeat(3);
        let data: [u8; 54] = [
            0x78, 0x01, 0x0b, 0xc9, 0x48, 0x55, 0x28, 0x2c, 0xcd, 0x4c, 0xce, 0x56, 0x48, 0x2a,
            0xca, 0x2f, 0xcf, 0x53, 0x48, 0xcb, 0xaf, 0x50, 0xc8, 0x2a, 0xcd, 0x2d, 0x28, 0x56,
            0xc8, 0x2f, 0x4b, 0x2d, 0x52, 0x28, 0x01, 0x4a, 0xe7, 0x24, 0x56, 0x55, 0x2a, 0xa4,
            0xe4, 0xa7, 0xeb, 0x85, 0xd0, 0x48, 0x2d, 0x00, 0x87, 0xcf, 0x30, 0x16,
        ];
        assert_eq!(inflate(&data).unwrap(), text);
    }
}
