//! Songbook layout and drawing - the print engine.
//!
//! Geometry (all values in PDF points, 72 per inch):
//!   row height 19, column gap 8, number column 40 (42 if numbers exceed
//!   9 999 999), title 65% and artist 35% of the rest, 35 pt top band,
//!   20 pt bottom band, rows per page coming from a hard-coded table.

use crate::fonts;
use crate::import::{Order, Song};
use crate::pdf::{self, Align, Color, Pdf};

pub const ROW_H: f32 = 19.0;
pub const GAP: f32 = 8.0;
pub const NUM_W: f32 = 40.0;
pub const TOP_BAND: f32 = 35.0;
pub const BOT_BAND: f32 = 20.0;
pub const LOGO_W: f32 = 119.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Paper {
    A4,
    Letter,
    Long,
    Legal,
}

impl Paper {
    pub fn parse(s: &str) -> Option<Paper> {
        match s {
            "a4" => Some(Paper::A4),
            "letter" => Some(Paper::Letter),
            "long" => Some(Paper::Long),
            "legal" => Some(Paper::Legal),
            _ => None,
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Paper::A4 => "a4",
            Paper::Letter => "letter",
            Paper::Long => "long",
            Paper::Legal => "legal",
        }
    }
    /// Size in inches, exactly as the app defines it (A4 is 8.3 in, not 8.27).
    pub fn size(self) -> (f32, f32) {
        match self {
            Paper::A4 => (8.3, 11.7),
            Paper::Letter => (8.5, 11.0),
            Paper::Long => (8.5, 13.0),
            Paper::Legal => (8.5, 14.0),
        }
    }
    fn index(self) -> usize {
        match self {
            Paper::A4 => 0,
            Paper::Letter => 1,
            Paper::Long => 2,
            Paper::Legal => 3,
        }
    }
}

pub const MARGINS: [f32; 5] = [0.20, 0.25, 0.30, 0.40, 0.50];

fn margin_index(m: f32) -> Option<usize> {
    MARGINS.iter().position(|v| (v - m).abs() < 0.001)
}

/// rows per page and y of the first row for every paper/margin combination.
const ROWS: [[(usize, f32); 5]; 4] = [
    [(39, 58.0), (39, 58.0), (39, 58.0), (38, 68.0), (37, 77.0)],
    [(37, 52.0), (36, 62.0), (36, 62.0), (35, 72.0), (35, 71.0)],
    [(44, 57.0), (44, 58.0), (44, 58.0), (43, 67.0), (42, 77.0)],
    [(48, 56.0), (48, 56.0), (47, 66.0), (47, 65.0), (46, 75.0)],
];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Sections {
    /// letter takes a row inside the flow
    Continuous,
    /// every new letter starts a new page, letter drawn in the header
    NewPage,
    /// no letters
    None,
}

impl Sections {
    pub fn parse(s: &str) -> Option<Sections> {
        match s {
            "continuous" => Some(Sections::Continuous),
            "newpage" => Some(Sections::NewPage),
            "none" => Some(Sections::None),
            _ => None,
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Sections::Continuous => "continuous",
            Sections::NewPage => "newpage",
            Sections::None => "none",
        }
    }
}

/// What is drawn around each song cell.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Borders {
    /// full grid, the default
    All,
    /// horizontal rules only, one line per row
    Rows,
    /// no grid at all, use with zebra rows
    None,
}

impl Borders {
    pub fn parse(s: &str) -> Option<Borders> {
        match s {
            "all" | "grid" => Some(Borders::All),
            "rows" | "lines" => Some(Borders::Rows),
            "none" | "off" => Some(Borders::None),
            _ => None,
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Borders::All => "all",
            Borders::Rows => "rows",
            Borders::None => "none",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SortKey {
    Title,
    Artist,
    Number,
}

impl SortKey {
    pub fn parse(s: &str) -> Option<SortKey> {
        match s {
            "title" => Some(SortKey::Title),
            "artist" => Some(SortKey::Artist),
            "number" => Some(SortKey::Number),
            _ => None,
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            SortKey::Title => "title",
            SortKey::Artist => "artist",
            SortKey::Number => "number",
        }
    }
}

pub struct Options {
    pub paper: Paper,
    pub margin: f32,
    pub columns: usize,
    pub order: Order,
    pub sort: SortKey,
    pub sections: Sections,
    pub no_artist: bool,
    pub name: String,
    pub header_left: String,
    pub header_center: String,
    pub header_right: String,
    pub footer_center: String,
    pub page_numbers: bool,
    pub mirror: bool,
    pub logo: Option<Vec<u8>>,
    pub bold: bool,
    pub scale: f32,
    pub num_size: f32,
    pub title_size: f32,
    pub artist_size: f32,
    pub letter_size: f32,
    pub header_size: f32,
    pub footer_size: f32,
    pub page_color: Color,
    pub line_color: Color,
    pub band_color: Color,
    pub band_opacity: u8,
    pub num_color: Color,
    pub title_color: Color,
    pub artist_color: Color,
    pub section_bg: Color,
    pub section_color: Color,
    pub header_color: Color,
    pub footer_color: Color,
    pub num_bg: Option<Color>,
    pub title_bg: Option<Color>,
    pub artist_bg: Option<Color>,
    pub borders: Borders,
    /// draw titles in upper case
    pub uppercase: bool,
    /// tint every second row
    pub zebra: bool,
    pub zebra_color: Color,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            paper: Paper::A4,
            margin: 0.25,
            columns: 3,
            order: Order::Auto,
            sort: SortKey::Title,
            sections: Sections::Continuous,
            no_artist: false,
            name: String::new(),
            header_left: String::new(),
            header_center: String::new(),
            header_right: String::new(),
            footer_center: String::new(),
            page_numbers: true,
            mirror: false,
            logo: None,
            bold: false,
            scale: 1.0,
            num_size: 7.5,
            title_size: 8.0,
            artist_size: 6.5,
            letter_size: 11.0,
            header_size: 9.0,
            footer_size: 6.5,
            page_color: Color::WHITE,
            line_color: Color::rgb8(0xBD, 0xBD, 0xBD),
            band_color: Color::BLACK,
            band_opacity: 34,
            num_color: Color::BLACK,
            title_color: Color::BLACK,
            artist_color: Color::BLACK,
            section_bg: Color::rgb8(0xEE, 0xEE, 0xEE),
            section_color: Color::BLACK,
            header_color: Color::BLACK,
            footer_color: Color::BLACK,
            num_bg: None,
            title_bg: None,
            artist_bg: None,
            borders: Borders::All,
            uppercase: false,
            zebra: false,
            zebra_color: Color::rgb8(0xF1, 0xF4, 0xF8),
        }
    }
}

#[derive(Debug)]
pub struct Summary {
    pub songs: usize,
    pub pages: usize,
    pub page_w: f32,
    pub page_h: f32,
    pub columns: usize,
    pub rows: usize,
    pub cells: [f32; 3],
    pub paper: &'static str,
}

#[derive(Clone, Copy, Debug)]
enum Item {
    Letter(char),
    Song(usize),
}

fn section_key(s: &Song, sort: SortKey) -> char {
    let v = match sort {
        SortKey::Artist => &s.artist,
        _ => &s.title,
    };
    match v.trim().chars().next() {
        Some(c) => {
            let up = c.to_uppercase().next().unwrap_or(c);
            if up.is_alphabetic() {
                up
            } else {
                '#'
            }
        }
        None => '#',
    }
}

/// A-Z first, then '#' for titles that start with a digit or symbol.
fn section_rank(c: char) -> (u8, char) {
    if c == '#' {
        (1, c)
    } else {
        (0, c)
    }
}

fn truncate_to_width(text: &str, size: f32, maxw: f32, bold: bool) -> (String, f32) {
    let mut sz = size;
    let floor = (size * 0.45).max(2.6);
    while fonts::text_width(text, sz, bold) > maxw && sz > floor {
        sz -= 0.25;
    }
    let mut s = text.to_string();
    while !s.is_empty() && fonts::text_width(&s, sz, bold) > maxw {
        s.pop();
    }
    (s, sz)
}

pub fn render(songs: &[Song], opt: &Options) -> Result<(Pdf, Summary), String> {
    let midx = margin_index(opt.margin)
        .ok_or_else(|| format!("margin must be one of 0.20 0.25 0.30 0.40 0.50 (got {})", opt.margin))?;
    if !(2..=4).contains(&opt.columns) {
        return Err("columns must be 2, 3 or 4".to_string());
    }
    if songs.is_empty() {
        return Err("no songs to print".to_string());
    }

    let (w_in, h_in) = opt.paper.size();
    let page_w = (w_in * 72.0).floor();
    let page_h = (h_in * 72.0).floor();
    let m = (opt.margin * 72.0).floor();
    let (rows, top) = ROWS[opt.paper.index()][midx];
    let bottom = top + rows as f32 * ROW_H;

    let cols = opt.columns;
    let col_w = (page_w - 2.0 * m - (cols as f32 - 1.0) * GAP) / cols as f32;
    let mut col_x = Vec::with_capacity(cols);
    let mut x = m;
    for _ in 0..cols {
        col_x.push(x.round());
        x += col_w + GAP;
    }

    let mut num_w = NUM_W;
    let rest = (col_w - NUM_W).floor();
    let mut title_w = (0.65 * rest).floor();
    let artist_w = (0.35 * rest).floor();
    let max_num = songs.iter().map(|s| s.num).max().unwrap_or(0);
    if max_num > 9_999_999 {
        num_w += 2.0;
        title_w -= 2.0;
    }
    let cells: [f32; 3] = if opt.no_artist {
        if opt.order == Order::Tna {
            [title_w + artist_w, num_w, 0.0]
        } else {
            [num_w, title_w + artist_w, 0.0]
        }
    } else if opt.order == Order::Tna {
        [title_w, num_w, artist_w]
    } else {
        [num_w, title_w, artist_w]
    };
    let total_w: f32 = cells.iter().sum();

    let scale = opt.scale.max(0.1);
    let sz_num = opt.num_size / scale;
    let sz_title = opt.title_size / scale;
    let sz_artist = opt.artist_size / scale;
    let sz_letter = opt.letter_size / scale;
    let sz_header = opt.header_size / scale;
    let sz_footer = opt.footer_size / scale;

    // ---------------- ordering and pagination -------------------------
    let mut order_idx: Vec<usize> = (0..songs.len()).collect();
    match opt.sort {
        SortKey::Title => order_idx.sort_by(|a, b| {
            let ka = (songs[*a].title.to_lowercase(), songs[*a].artist.to_lowercase(), songs[*a].num);
            let kb = (songs[*b].title.to_lowercase(), songs[*b].artist.to_lowercase(), songs[*b].num);
            ka.cmp(&kb)
        }),
        SortKey::Artist => order_idx.sort_by(|a, b| {
            let ka = (songs[*a].artist.to_lowercase(), songs[*a].title.to_lowercase(), songs[*a].num);
            let kb = (songs[*b].artist.to_lowercase(), songs[*b].title.to_lowercase(), songs[*b].num);
            ka.cmp(&kb)
        }),
        SortKey::Number => order_idx.sort_by(|a, b| {
            let ka = (songs[*a].num, songs[*a].title.to_lowercase());
            let kb = (songs[*b].num, songs[*b].title.to_lowercase());
            ka.cmp(&kb)
        }),
    }
    if opt.sections == Sections::Continuous {
        order_idx.sort_by_key(|i| section_rank(section_key(&songs[*i], opt.sort)));
    }

    let mut items: Vec<Item> = Vec::new();
    if opt.sections == Sections::None {
        items.extend(order_idx.iter().map(|i| Item::Song(*i)));
    } else {
        let mut last = None;
        for i in order_idx.iter().copied() {
            let k = section_key(&songs[i], opt.sort);
            if last != Some(k) {
                items.push(Item::Letter(k));
                last = Some(k);
            }
            items.push(Item::Song(i));
        }
    }

    let per_page = rows * cols;
    let mut pages: Vec<Vec<Item>> = Vec::new();
    if opt.sections == Sections::NewPage {
        let mut cur: Vec<Item> = Vec::new();
        for it in items {
            match it {
                Item::Letter(c) => {
                    while !cur.is_empty() {
                        let take = cur.len().min(per_page);
                        pages.push(cur.drain(..take).collect());
                    }
                    cur.push(Item::Letter(c));
                }
                Item::Song(_) => {
                    cur.push(it);
                    if cur.len() >= per_page {
                        pages.push(std::mem::take(&mut cur));
                    }
                }
            }
        }
        while !cur.is_empty() {
            let take = cur.len().min(per_page);
            pages.push(cur.drain(..take).collect());
        }
    } else {
        let mut cur: Vec<Item> = Vec::new();
        for it in items {
            cur.push(it);
            if cur.len() == per_page {
                pages.push(std::mem::take(&mut cur));
            }
        }
        if !cur.is_empty() || pages.is_empty() {
            pages.push(cur);
        }
    }
    if pages.is_empty() {
        pages.push(Vec::new());
    }

    // ---------------- drawing helpers --------------------------------
    let mut pdf = Pdf::new();
    let logo_index = match &opt.logo {
        Some(p) => {
            let (lw, lh, rgb) = pdf::load_png(p, Color::WHITE)
                .map_err(|e| format!("logo: {}", e))?;
            Some(pdf.add_image("Im1", lw, lh, rgb))
        }
        None => None,
    };

    let band_top = opt.band_color.blend(opt.page_color, opt.band_opacity as f32 / 255.0);
    let band_bot = opt.band_color.blend(opt.page_color, opt.band_opacity as f32 / 255.0);

    let total_pages = pages.len();
    for (pno, page_items) in pages.iter().enumerate() {
        let pno = pno + 1;
        pdf.page(page_w, page_h);
        pdf.fill_rect(0.0, 0.0, page_w, page_h, opt.page_color);

        // column background bands, drawn before the rows
        for c in 0..cols {
            for ci in 0..3 {
                let bg = match ci {
                    0 => opt.num_bg,
                    1 => opt.title_bg,
                    _ => opt.artist_bg,
                };
                if cells[ci] <= 0.0 {
                    continue;
                }
                if let Some(bg) = bg {
                    let cw: f32 = cells[..ci].iter().sum();
                    pdf.fill_rect(col_x[c] + cw, top, cells[ci], bottom - top, bg);
                }
            }
        }

        let flow: Vec<Item> = if opt.sections == Sections::NewPage {
            page_items.iter().copied().filter(|i| matches!(i, Item::Song(_))).collect()
        } else {
            page_items.clone()
        };
        let mut pos = 0usize;
        'outer: for c in 0..cols {
            for r in 0..rows {
                if pos >= flow.len() {
                    break 'outer;
                }
                let item = flow[pos];
                pos += 1;
                let y_top = top + r as f32 * ROW_H;
                match item {
                    Item::Letter(ch) => {
                        let mut txt = ch.to_string();
                        pdf.fill_rect(col_x[c] - 1.0, y_top - 1.0, total_w - 2.0, ROW_H + 2.0, opt.section_bg);
                        if opt.borders != Borders::None {
                            pdf.stroke_rect(col_x[c] - 1.0, y_top - 1.0, total_w - 2.0, ROW_H + 2.0, opt.section_color, 0.2);
                        }
                        let (fitted, sz) = truncate_to_width(&txt, sz_letter, total_w - 2.0, true);
                        txt = fitted;
                        pdf.text(
                            &txt,
                            col_x[c] - 1.0 + (total_w - 2.0) / 2.0,
                            y_top + ROW_H / 2.0 + 0.45 * sz,
                            sz,
                            opt.section_color,
                            true,
                            Align::Center,
                        );
                    }
                    Item::Song(i) => {
                        let song = &songs[i];
                        if opt.zebra && r % 2 == 1 {
                            pdf.fill_rect(col_x[c], y_top, total_w, ROW_H, opt.zebra_color);
                        }
                        match opt.borders {
                            Borders::All => {
                                let mut cx = col_x[c];
                                for ci in 0..3 {
                                    if cells[ci] <= 0.0 {
                                        continue;
                                    }
                                    pdf.stroke_rect(cx, y_top, cells[ci], ROW_H, opt.line_color, 0.2);
                                    cx += cells[ci];
                                }
                            }
                            Borders::Rows => {
                                pdf.line(col_x[c], y_top, col_x[c] + total_w, y_top, opt.line_color, 0.2);
                            }
                            Borders::None => {}
                        }
                        let y_bot = y_top + ROW_H;
                        let title: std::borrow::Cow<str> = if opt.uppercase {
                            song.title.to_uppercase().into()
                        } else {
                            std::borrow::Cow::Borrowed(song.title.as_str())
                        };
                        if opt.order == Order::Tna {
                            draw_text_cell(&mut pdf, &title, col_x[c] + cells[0], cells[1], y_bot, sz_title, opt.title_color, opt.bold);
                            let (txt, sz) = truncate_to_width(&song.num.to_string(), sz_num, cells[0] - 1.0, opt.bold);
                            pdf.text(&txt, col_x[c] + cells[0] / 2.0, y_top + ROW_H / 2.0 + 0.45 * sz, sz, opt.num_color, opt.bold, Align::Center);
                        } else {
                            let (txt, sz) = truncate_to_width(&song.num.to_string(), sz_num, cells[0] - 1.0, opt.bold);
                            pdf.text(&txt, col_x[c] + cells[0] / 2.0, y_top + ROW_H / 2.0 + 0.45 * sz, sz, opt.num_color, opt.bold, Align::Center);
                            draw_text_cell(&mut pdf, &title, col_x[c] + cells[0], cells[1], y_bot, sz_title, opt.title_color, opt.bold);
                        }
                        if !opt.no_artist {
                            draw_text_cell(&mut pdf, &song.artist, col_x[c] + cells[0] + cells[1], cells[2], y_bot, sz_artist, opt.artist_color, opt.bold);
                        }
                    }
                }
            }
        }

        // ---------------- header and footer --------------------------
        pdf.fill_rect(m, m, page_w - 2.0 * m, TOP_BAND, band_top);
        pdf.fill_rect(m, page_h - m - BOT_BAND, page_w - 2.0 * m, BOT_BAND, band_bot);

        let base_top = m + 17.5 + fonts::text_height(sz_header) / 2.0 - 1.0;
        let base_bot = page_h - m - 11.0 + fonts::text_height(sz_footer) / 2.0;
        let maxw = page_w - 2.0 * m - 10.0;

        let mirror = opt.mirror && pno % 2 == 0;
        let (mut left_txt, mut right_txt) = (opt.header_left.clone(), opt.header_right.clone());
        if mirror {
            std::mem::swap(&mut left_txt, &mut right_txt);
        }
        let page_letter = if opt.sections == Sections::NewPage {
            page_items.iter().find_map(|i| match i {
                Item::Letter(c) => Some(*c),
                _ => None,
            })
        } else {
            None
        };
        if let Some(ch) = page_letter {
            let (txt, sz) = truncate_to_width(&ch.to_string(), sz_header * 1.4, maxw / 3.0, true);
            pdf.text(&txt, m + 5.0, base_top, sz, opt.header_color, true, Align::Left);
        } else if !left_txt.is_empty() {
            let (txt, sz) = truncate_to_width(&left_txt, sz_header, maxw, opt.bold);
            pdf.text(&txt, m + 5.0, base_top, sz, opt.header_color, opt.bold, Align::Left);
        }
        if !right_txt.is_empty() {
            let (txt, sz) = truncate_to_width(&right_txt, sz_header, maxw, opt.bold);
            pdf.text(&txt, page_w - m - 5.0, base_top, sz, opt.header_color, opt.bold, Align::Right);
        }
        let center = if opt.header_center.is_empty() { opt.name.as_str() } else { opt.header_center.as_str() };
        if !center.is_empty() {
            let (txt, sz) = truncate_to_width(center, sz_header, maxw, opt.bold);
            pdf.text(&txt, page_w / 2.0, base_top, sz, opt.header_color, opt.bold, Align::Center);
        }
        if !opt.footer_center.is_empty() {
            let (txt, sz) = truncate_to_width(&opt.footer_center, sz_footer, maxw, opt.bold);
            pdf.text(&txt, page_w / 2.0, base_bot, sz, opt.footer_color, opt.bold, Align::Center);
        }
        if opt.page_numbers {
            let s = format!("{} / {}", pno, total_pages);
            if mirror {
                pdf.text(&s, m + 5.0, base_bot, sz_footer, opt.footer_color, opt.bold, Align::Left);
            } else {
                pdf.text(&s, page_w - m - 5.0, base_bot, sz_footer, opt.footer_color, opt.bold, Align::Right);
            }
        }
        if let Some(idx) = logo_index {
            let y = page_h - m - BOT_BAND;
            let x = if mirror { page_w - m - 5.0 - LOGO_W } else { m + 5.0 };
            pdf.image(idx, x, y, LOGO_W, BOT_BAND);
        }
    }

    let summary = Summary {
        songs: songs.len(),
        pages: total_pages,
        page_w,
        page_h,
        columns: cols,
        rows,
        cells,
        paper: opt.paper.name(),
    };
    Ok((pdf, summary))
}

/// Draw title or artist text with shrink-to-fit and a two line fallback.
fn draw_text_cell(pdf: &mut Pdf, text: &str, x: f32, cell_w: f32, y_bot: f32, size: f32, color: Color, bold: bool) {
    if text.is_empty() {
        return;
    }
    let maxw = cell_w - 3.0;
    let floor = (size * 0.45).max(2.6);
    let mut sz = size;
    while fonts::text_width(text, sz, bold) > maxw && sz > floor {
        sz -= 0.25;
    }
    if fonts::text_width(text, sz, bold) <= maxw {
        let (txt, s) = truncate_to_width(text, sz, maxw, bold);
        pdf.text(&txt, x + 2.0, y_bot - ROW_H / 2.0 + 0.45 * s, s, color, bold, Align::Left);
        return;
    }
    // split at the last space or hyphen that still fits
    let mut best: Option<usize> = None;
    let mut acc = String::new();
    for (i, ch) in text.char_indices() {
        acc.push(ch);
        if ch == ' ' || ch == '-' {
            best = Some(i);
        }
        if fonts::text_width(&acc, sz, bold) > maxw {
            break;
        }
    }
    let (line1, line2) = match best {
        Some(i) => (&text[..i + 1], &text[i + 1..]),
        None => {
            let cut = acc.len();
            (&text[..cut], &text[cut..])
        }
    };
    if line2.trim().is_empty() {
        let (txt, s) = truncate_to_width(text, sz, maxw, bold);
        pdf.text(&txt, x + 2.0, y_bot - ROW_H / 2.0 + 0.45 * s, s, color, bold, Align::Left);
    } else {
        let (l1, s1) = truncate_to_width(line1.trim(), sz, maxw, bold);
        let (l2, s2) = truncate_to_width(line2.trim(), sz, maxw, bold);
        pdf.text(&l1, x + 2.0, y_bot - ROW_H * 0.58, s1, color, bold, Align::Left);
        pdf.text(&l2, x + 2.0, y_bot - ROW_H * 0.16, s2, color, bold, Align::Left);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::import::DEFAULT_ARTIST;

    fn song(num: i64, title: &str, artist: &str) -> Song {
        Song {
            num,
            title: title.to_string(),
            artist: artist.to_string(),
            genre: String::new(),
            stype: String::new(),
            release: String::new(),
            line: 1,
        }
    }

    #[test]
    fn a4_geometry() {
        let songs = vec![song(1, "A", "B")];
        let (_, s) = render(&songs, &Options::default()).unwrap();
        assert_eq!(s.page_w, 597.0);
        assert_eq!(s.page_h, 842.0);
        assert_eq!(s.rows, 39);
        assert_eq!(s.columns, 3);
        assert_eq!(s.cells, [40.0, 91.0, 49.0]);
    }

    #[test]
    fn page_count_grows_with_songs() {
        let songs: Vec<Song> = (0..118).map(|i| song(i, &format!("T{}", i), "A")).collect();
        let (_, s) = render(&songs, &Options::default()).unwrap();
        assert_eq!(s.pages, 2); // 117 slots on page one, one song spills over
    }

    #[test]
    fn no_artist_merges_columns() {
        let songs = vec![song(1, "Title", "Artist")];
        let mut o = Options::default();
        o.no_artist = true;
        let (_, s) = render(&songs, &o).unwrap();
        assert_eq!(s.cells[2], 0.0);
        assert_eq!(s.cells[1], 140.0);
    }

    #[test]
    fn newpage_sections_start_new_pages() {
        let mut songs = Vec::new();
        for (i, (t, a)) in [("Apple", "A"), ("Banana", "B"), ("Cherry", "C")].iter().enumerate() {
            songs.push(song(i as i64, t, a));
        }
        let mut o = Options::default();
        o.sections = Sections::NewPage;
        let (_, s) = render(&songs, &o).unwrap();
        assert_eq!(s.pages, 3);
    }

    #[test]
    fn sections_and_defaults_parse() {
        assert_eq!(Sections::parse("newpage"), Some(Sections::NewPage));
        assert_eq!(Sections::parse("nope"), None);
        assert_eq!(Paper::parse("legal"), Some(Paper::Legal));
        assert_eq!(SortKey::parse("artist"), Some(SortKey::Artist));
        assert_eq!(Order::parse("tna"), Some(Order::Tna));
        assert_eq!(DEFAULT_ARTIST, "(unknown)");
    }
}
