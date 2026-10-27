//! Shared engine: parse a song list, lay it out and produce PDF bytes.
//!
//! The CLI (src/main.rs) and the Android app (JNI, see android.rs) both go
//! through render_text, so a songbook looks the same everywhere.

pub mod fonts;
pub mod import;
pub mod pdf;
pub mod render;

#[cfg(feature = "android")]
mod android;

use std::fmt::Write as _;

use import::Order;
use pdf::Color;
use render::{Borders, Options, Paper, Sections, SortKey};

/// Named looks accepted by `style=` and `--style`.
pub const STYLES: [&str; 3] = ["default", "platinum", "tj"];

/// The stock look, spelled out so `style=default` resets a previous preset.
const STYLE_DEFAULT: &str = "\
paper=a4
margin=0.25
columns=3
borders=all
uppercase=0
zebra=0
line_color=BDBDBD
band_color=000000
band_opacity=34
header_color=000000
footer_color=000000
section_bg=EEEEEE
section_color=000000
num_color=000000
title_color=000000
artist_color=000000
num_size=7.5
title_size=8
artist_size=6.5
letter_size=11
header_size=9
footer_size=6.5
";

/// Platinum: dark bands, hairline row rules, upper case titles.
const STYLE_PLATINUM: &str = "\
paper=letter
margin=0.30
columns=3
borders=rows
uppercase=1
zebra=0
line_color=C9CED6
band_color=14181F
band_opacity=235
header_color=FFFFFF
footer_color=FFFFFF
section_bg=14181F
section_color=FFFFFF
num_color=14181F
title_color=14181F
artist_color=4B5563
num_size=7.4
title_size=7.6
artist_size=6.3
letter_size=10
header_size=9.5
";

/// TJ Media: dense four column listing, no grid, zebra rows, black band.
const STYLE_TJ: &str = "\
paper=a4
margin=0.25
columns=4
borders=none
uppercase=0
zebra=1
zebra_color=EEF2F7
line_color=DFE4EA
band_color=0A0A0A
band_opacity=255
header_color=FFFFFF
footer_color=FFFFFF
section_bg=0A0A0A
section_color=FFFFFF
num_color=0A0A0A
title_color=111111
artist_color=6B7280
num_size=7.2
title_size=7.4
artist_size=6.2
letter_size=10
";

/// Apply a named look. Later settings always win over the preset.
pub fn apply_style(opt: &mut Options, name: &str) -> Result<(), String> {
    let body = match name.trim() {
        "default" | "classic" | "none" => STYLE_DEFAULT,
        "platinum" => STYLE_PLATINUM,
        "tj" | "tjmedia" | "tj-media" => STYLE_TJ,
        other => {
            return Err(format!(
                "style: '{}' is not one of {}",
                other,
                STYLES.join(", ")
            ))
        }
    };
    apply_config(opt, body)
}

/// Result of one render, plus the numbers the UI wants to show.
#[derive(Debug)]
pub struct Output {
    pub pdf: Vec<u8>,
    pub songs: usize,
    pub pages: usize,
    pub rejected: usize,
    pub user_content: usize,
    pub page_w: f32,
    pub page_h: f32,
    pub title: String,
    pub summary: String,
}

/// Apply a key=value configuration (one per line) on top of the defaults.
pub fn apply_config(opt: &mut Options, cfg: &str) -> Result<(), String> {
    for (lineno, line) in cfg.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (key, value) = line
            .split_once('=')
            .ok_or_else(|| format!("config line {}: expected key=value", lineno + 1))?;
        let value = value.trim();
        let num = |v: &str| -> Result<f32, String> {
            v.parse::<f32>()
                .map_err(|_| format!("{}: '{}' is not a number", key.trim(), v))
        };
        let flag = |v: &str| -> bool { !(v == "0" || v.eq_ignore_ascii_case("false")) };
        match key.trim() {
            "paper" => opt.paper = Paper::parse(value).ok_or_else(|| format!("paper: '{}'", value))?,
            "margin" => opt.margin = num(value)?,
            "columns" => {
                opt.columns = value.parse::<usize>().map_err(|_| format!("columns: '{}'", value))?
            }
            "order" => opt.order = Order::parse(value).ok_or_else(|| format!("order: '{}'", value))?,
            "sort" => opt.sort = SortKey::parse(value).ok_or_else(|| format!("sort: '{}'", value))?,
            "sections" => {
                opt.sections = Sections::parse(value).ok_or_else(|| format!("sections: '{}'", value))?
            }
            "artist" => opt.no_artist = !flag(value),
            "name" => opt.name = value.to_string(),
            "header_left" => opt.header_left = value.to_string(),
            "header_center" => opt.header_center = value.to_string(),
            "header_right" => opt.header_right = value.to_string(),
            "footer_center" => opt.footer_center = value.to_string(),
            "page_numbers" => opt.page_numbers = flag(value),
            "mirror" => opt.mirror = flag(value),
            "bold" => opt.bold = flag(value),
            "scale" => opt.scale = num(value)?,
            "num_size" => opt.num_size = num(value)?,
            "title_size" => opt.title_size = num(value)?,
            "artist_size" => opt.artist_size = num(value)?,
            "letter_size" => opt.letter_size = num(value)?,
            "header_size" => opt.header_size = num(value)?,
            "footer_size" => opt.footer_size = num(value)?,
            "page_color" => opt.page_color = hex(key, value)?,
            "line_color" => opt.line_color = hex(key, value)?,
            "band_color" => opt.band_color = hex(key, value)?,
            "band_opacity" => {
                opt.band_opacity =
                    value.parse::<u8>().map_err(|_| format!("band_opacity: '{}'", value))?
            }
            "num_color" => opt.num_color = hex(key, value)?,
            "title_color" => opt.title_color = hex(key, value)?,
            "artist_color" => opt.artist_color = hex(key, value)?,
            "section_bg" => opt.section_bg = hex(key, value)?,
            "section_color" => opt.section_color = hex(key, value)?,
            "header_color" => opt.header_color = hex(key, value)?,
            "footer_color" => opt.footer_color = hex(key, value)?,
            "num_bg" => opt.num_bg = Some(hex(key, value)?),
            "title_bg" => opt.title_bg = Some(hex(key, value)?),
            "artist_bg" => opt.artist_bg = Some(hex(key, value)?),
            "style" => apply_style(opt, value)?,
            "borders" => {
                opt.borders = Borders::parse(value).ok_or_else(|| format!("borders: '{}'", value))?
            }
            "uppercase" => opt.uppercase = flag(value),
            "zebra" => opt.zebra = flag(value),
            "zebra_color" => opt.zebra_color = hex(key, value)?,
            other => return Err(format!("unknown config key '{}'", other)),
        }
    }
    Ok(())
}

fn hex(key: &str, value: &str) -> Result<Color, String> {
    Color::from_hex(value).ok_or_else(|| format!("{}: '{}' is not a colour", key.trim(), value))
}

/// Parse input, lay it out with opt and return the PDF plus a report.
pub fn render_text(input: &str, opt: &Options) -> Result<Output, String> {
    let (songs, report) = import::parse(input, opt.order);
    if songs.is_empty() {
        return Err(if report.rejected() > 0 {
            format!("no valid songs - {} lines rejected", report.rejected())
        } else {
            "no songs found in that file".to_string()
        });
    }
    let (pdf, summary) = render::render(&songs, opt)?;
    let bytes = pdf.bytes(if opt.name.is_empty() { "Songbook" } else { &opt.name });
    Ok(Output {
        pdf: bytes,
        songs: songs.len(),
        pages: summary.pages,
        rejected: report.rejected(),
        user_content: report.user_content,
        page_w: summary.page_w,
        page_h: summary.page_h,
        title: opt.name.clone(),
        summary: report.render(6),
    })
}

/// Convenience wrapper: defaults + cfg + text.
pub fn render_with_config(input: &str, cfg: &str) -> Result<Output, String> {
    let mut opt = Options::default();
    apply_config(&mut opt, cfg)?;
    render_text(input, &opt)
}

/// Binary packet used across JNI:
/// u32be meta_len | meta (utf8, key=value lines) | pdf bytes.
/// On failure the meta contains error=<message> and there is no payload.
pub fn packet(result: Result<Output, String>) -> Vec<u8> {
    let (meta, body) = match result {
        Ok(out) => {
            let mut m = String::new();
            let _ = writeln!(m, "songs={}", out.songs);
            let _ = writeln!(m, "pages={}", out.pages);
            let _ = writeln!(m, "rejected={}", out.rejected);
            let _ = writeln!(m, "user_content={}", out.user_content);
            let _ = writeln!(m, "page_w={}", out.page_w);
            let _ = writeln!(m, "page_h={}", out.page_h);
            let _ = writeln!(m, "title={}", escape_value(&out.title));
            let _ = writeln!(m, "summary={}", escape_value(&out.summary));
            (m, out.pdf)
        }
        Err(msg) => {
            let mut m = String::new();
            let _ = writeln!(m, "error={}", escape_value(&msg));
            (m, Vec::new())
        }
    };
    let mut buf = Vec::with_capacity(4 + meta.len() + body.len());
    buf.extend_from_slice(&(meta.len() as u32).to_be_bytes());
    buf.extend_from_slice(meta.as_bytes());
    buf.extend_from_slice(&body);
    buf
}

fn escape_value(s: &str) -> String {
    s.replace('\\', "\\\\").replace('\n', "\\n").replace('\r', "")
}

/// Counts for the "file loaded" line, without rendering a page.
pub struct Inspect {
    pub songs: usize,
    pub rejected: usize,
    pub user_content: usize,
    pub summary: String,
}

pub fn inspect(input: &str, order: Order) -> Inspect {
    let (songs, report) = import::parse(input, order);
    Inspect {
        songs: songs.len(),
        rejected: report.rejected(),
        user_content: report.user_content,
        summary: report.render(4),
    }
}

/// Same key=value meta format as packet(), without the PDF payload.
pub fn inspect_meta(input: &str, order: Order) -> String {
    let i = inspect(input, order);
    let mut m = String::new();
    let _ = writeln!(m, "songs={}", i.songs);
    let _ = writeln!(m, "rejected={}", i.rejected);
    let _ = writeln!(m, "user_content={}", i.user_content);
    let _ = writeln!(m, "summary={}", escape_value(&i.summary));
    m
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_overrides_defaults() {
        let mut o = Options::default();
        apply_config(
            &mut o,
            "paper=legal
columns=2
margin=0.4
sections=newpage
mirror=1
name=Test
",
        )
        .unwrap();
        assert_eq!(o.paper, Paper::Legal);
        assert_eq!(o.columns, 2);
        assert_eq!(o.sections, Sections::NewPage);
        assert!(o.mirror);
        assert_eq!(o.name, "Test");
    }

    #[test]
    fn styles_change_the_look() {
        let mut o = Options::default();
        apply_config(&mut o, "style=platinum").unwrap();
        assert_eq!(o.borders, Borders::Rows);
        assert!(o.uppercase);
        assert_eq!(o.columns, 3);
        assert_eq!(o.paper, Paper::Letter);

        // explicit keys win over the preset
        apply_config(&mut o, "style=tj\ncolumns=2").unwrap();
        assert_eq!(o.borders, Borders::None);
        assert!(o.zebra);
        assert_eq!(o.columns, 2);

        // the stock preset puts everything back
        apply_config(&mut o, "style=default").unwrap();
        let d = Options::default();
        assert_eq!(o.borders, d.borders);
        assert_eq!(o.zebra, d.zebra);
        assert_eq!(o.columns, d.columns);
        assert_eq!(o.paper, d.paper);
        assert_eq!(o.title_size, d.title_size);
        assert!(!o.uppercase);
    }

    #[test]
    fn every_style_renders() {
        for style in STYLES {
            let cfg = format!("style={}", style);
            let out = render_with_config("1|A|B\n2|C|D\n", &cfg).unwrap_or_else(|e| panic!("{}: {}", style, e));
            assert!(out.pdf.starts_with(b"%PDF-"));
            assert_eq!(out.songs, 2);
        }
    }

    #[test]
    fn unknown_style_is_rejected() {
        let mut o = Options::default();
        assert!(apply_config(&mut o, "style=vaporwave").is_err());
    }

    #[test]
    fn config_rejects_unknown_keys() {
        let mut o = Options::default();
        assert!(apply_config(&mut o, "nope=1").is_err());
        assert!(apply_config(&mut o, "columns=zzz").is_err());
        assert!(apply_config(&mut o, "line_color=xyz").is_err());
    }

    #[test]
    fn packet_carries_meta_then_pdf() {
        let out = render_with_config("1|A|B
2|C|D
", "name=Test").unwrap();
        let buf = packet(Ok(out));
        let len = u32::from_be_bytes([buf[0], buf[1], buf[2], buf[3]]) as usize;
        let meta = std::str::from_utf8(&buf[4..4 + len]).unwrap();
        assert!(meta.contains("songs=2"));
        assert!(meta.contains("pages=1"));
        assert_eq!(&buf[4 + len..4 + len + 5], b"%PDF-");
    }

    #[test]
    fn packet_error_has_no_payload() {
        let buf = packet(Err("boom".to_string()));
        let len = u32::from_be_bytes([buf[0], buf[1], buf[2], buf[3]]) as usize;
        assert_eq!(buf.len(), 4 + len);
        assert!(std::str::from_utf8(&buf[4..]).unwrap().contains("error=boom"));
    }

    #[test]
    fn inspect_counts_rejected_lines() {
        let meta = inspect_meta("1|A|B
abc|C|D
", Order::Auto);
        assert!(meta.contains("songs=1"));
        assert!(meta.contains("rejected=1"));
    }

    #[test]
    fn empty_list_is_an_error() {
        assert!(render_with_config("", "").is_err());
    }
}
