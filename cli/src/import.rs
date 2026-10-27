//! Song list import: the app's 3/6 column format plus ordinary CSV.
//!
//! Import rules of the songbook format:
//!   * delimiter sniffing on the first lines (pipe, TAB, and also ; and ,)
//!   * column orders number|title|artist, title|number|artist, artist|title|number
//!   * rejected lines are collected instead of aborting the whole import

/// Column used when the artist field is empty (app: main._v24_1).
pub const DEFAULT_ARTIST: &str = "(unknown)";

#[derive(Clone, Debug, PartialEq)]
pub struct Song {
    pub num: i64,
    pub title: String,
    pub artist: String,
    pub genre: String,
    pub stype: String,
    pub release: String,
    /// 1-based line number in the source file, for the rejection report.
    pub line: usize,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Order {
    Auto,
    /// number | title | artist
    Nta,
    /// title | number | artist
    Tna,
    /// artist | title | number
    Atn,
}

impl Order {
    pub fn parse(s: &str) -> Option<Order> {
        match s {
            "auto" => Some(Order::Auto),
            "nta" => Some(Order::Nta),
            "tna" => Some(Order::Tna),
            "atn" => Some(Order::Atn),
            _ => None,
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Order::Auto => "auto",
            Order::Nta => "nta",
            Order::Tna => "tna",
            Order::Atn => "atn",
        }
    }
}

#[derive(Default, Debug)]
pub struct Report {
    pub lines: usize,
    /// fewer than three columns, or an empty number/title
    pub invalid: Vec<(usize, String)>,
    pub no_title: Vec<(usize, String)>,
    pub bad_number: Vec<(usize, String)>,
    pub duplicate: Vec<(usize, String)>,
    pub user_content: usize,
}

impl Report {
    pub fn rejected(&self) -> usize {
        self.invalid.len() + self.no_title.len() + self.bad_number.len() + self.duplicate.len()
    }

    /// Human readable summary, following the wording of the app's dialog.
    pub fn render(&self, max_lines: usize) -> String {
        let mut out = String::new();
        let groups: [(&str, &Vec<(usize, String)>); 4] = [
            ("Invalid line", &self.invalid),
            ("No title", &self.no_title),
            ("Invalid number", &self.bad_number),
            ("Number exists", &self.duplicate),
        ];
        for (label, list) in groups {
            if list.is_empty() {
                continue;
            }
            out.push_str(&format!("{} ({}):\n", label, list.len()));
            for (line, raw) in list.iter().take(max_lines) {
                out.push_str(&format!("  line {}: {}\n", line, truncate(raw, 100)));
            }
            if list.len() > max_lines {
                out.push_str("  ... more.\n");
            }
        }
        out
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let mut t: String = s.chars().take(max).collect();
        t.push('\u{2026}');
        t
    }
}

/// Pick the delimiter that yields the most 3+ column lines.
pub fn detect_delimiter(lines: &[&str]) -> char {
    let sample: Vec<&str> =
        lines.iter().copied().filter(|l| !l.trim().is_empty()).take(50).collect();
    if sample.is_empty() {
        return '|';
    }
    let mut best = '|';
    let mut best_score = 0usize;
    for d in ['|', '\t', ';', ','] {
        let counts: Vec<usize> = sample.iter().map(|l| split_fields(l, d).len()).collect();
        let min = counts.iter().copied().min().unwrap_or(0);
        let sum: usize = counts.iter().sum();
        if min < 2 || sum < counts.len() * 3 {
            continue;
        }
        let score = counts.iter().filter(|c| **c >= 3).count();
        if score > best_score {
            best = d;
            best_score = score;
        }
    }
    best
}

/// Spreadsheet apps write text-only columns as formulas, `="100034"`.
/// Song lists exported from song managers arrive wrapped like that, so the
/// wrapper is removed here instead of turning every number into "Invalid".
fn clean_literal(field: &str) -> String {
    let f = field.trim();
    match f.strip_prefix("=\"").and_then(|r| r.strip_suffix('"')) {
        Some(inner) => inner.trim().to_string(),
        None => f.trim_matches('"').trim().to_string(),
    }
}

/// Split one line. Quoted values are honoured for ',' and ';' (RFC 4180),
/// pipe/TAB files are split literally (quotes stripped).
pub fn split_fields(line: &str, delim: char) -> Vec<String> {
    if delim == ',' || delim == ';' {
        let mut fields = Vec::new();
        let mut cur = String::new();
        let mut in_quotes = false;
        let mut chars = line.chars().peekable();
        while let Some(c) = chars.next() {
            match c {
                // Excel/Sheets wrapper: ="value" -> value
                '=' if cur.is_empty() && !in_quotes && chars.peek() == Some(&'"') => {
                    chars.next();
                    in_quotes = true;
                }
                '"' if in_quotes && chars.peek() == Some(&'"') => {
                    cur.push('"');
                    chars.next();
                }
                '"' => in_quotes = !in_quotes,
                c if c == delim && !in_quotes => {
                    fields.push(cur.trim().to_string());
                    cur = String::new();
                }
                c => cur.push(c),
            }
        }
        fields.push(cur.trim().to_string());
        fields
    } else {
        line.split(delim).map(clean_literal).collect()
    }
}

/// First line of a spreadsheet export is a caption, not a song.
fn looks_like_header(cols: &[String]) -> bool {
    if cols.len() < 2 {
        return false;
    }
    let first = cols[0].trim().to_ascii_lowercase();
    let second = cols[1].trim().to_ascii_lowercase();
    let number = matches!(first.as_str(), "number" | "no" | "num" | "n" | "#" | "song number");
    let title = matches!(second.as_str(), "title" | "song" | "song title" | "name");
    number && title
}

fn looks_numeric(s: &str) -> bool {
    s.parse::<f64>().is_ok()
}

/// Parse a song list.
pub fn parse(text: &str, order: Order) -> (Vec<Song>, Report) {
    let text = text.trim_start_matches('\u{feff}');
    let lines: Vec<&str> = text.lines().collect();
    let delim = detect_delimiter(&lines);
    let mut header_allowed = true;

    let mut songs: Vec<Song> = Vec::new();
    let mut report = Report::default();
    let mut seen: std::collections::HashSet<i64> = std::collections::HashSet::new();

    for (idx, raw) in lines.iter().enumerate() {
        let line_no = idx + 1;
        if raw.trim().is_empty() {
            continue;
        }
        let cols = split_fields(raw, delim);
        if header_allowed {
            header_allowed = false;
            if looks_like_header(&cols) {
                continue;
            }
        }
        report.lines += 1;
        if cols.len() < 3 {
            report.invalid.push((line_no, raw.to_string()));
            continue;
        }
        let (c0, c1, c2) = (cols[0].as_str(), cols[1].as_str(), cols[2].as_str());
        if c0.is_empty() || c1.is_empty() {
            report.invalid.push((line_no, raw.to_string()));
            continue;
        }
        let c2 = if c2.is_empty() { DEFAULT_ARTIST } else { c2 };

        let o = match order {
            Order::Auto => {
                if looks_numeric(c0) {
                    Order::Nta
                } else if looks_numeric(c1) {
                    Order::Tna
                } else if looks_numeric(c2) {
                    Order::Atn
                } else {
                    Order::Nta
                }
            }
            other => other,
        };
        let (num_s, title_s, artist_s) = match o {
            Order::Nta => (c0, c1, c2),
            Order::Tna => (c1, c0, c2),
            Order::Atn => (c2, c1, c0),
            Order::Auto => unreachable!(),
        };

        let num = match num_s.parse::<f64>() {
            Ok(v) if v.is_finite() => v as i64,
            _ => {
                report.bad_number.push((line_no, raw.to_string()));
                continue;
            }
        };
        if title_s.is_empty() {
            report.no_title.push((line_no, raw.to_string()));
            continue;
        }
        if !seen.insert(num) {
            report.duplicate.push((line_no, raw.to_string()));
            continue;
        }
        let release = cols.get(5).cloned().unwrap_or_default();
        if release == "user content" {
            report.user_content += 1;
            continue;
        }
        let mut artist = artist_s.to_string();
        if !artist.contains(',') {
            artist = artist.replace(';', ",");
        }
        songs.push(Song {
            num,
            title: title_s.to_string(),
            artist,
            genre: cols.get(3).cloned().unwrap_or_default(),
            stype: cols.get(4).cloned().unwrap_or_default(),
            release,
            line: line_no,
        });
    }

    (songs, report)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_spreadsheet_exports() {
        // exactly what a SongHub export looks like: caption row, every value
        // wrapped in an Excel text formula, six columns
        let text = concat!(
            "NUMBER,TITLE,ARTIST,CATEGORY,TYPE,UPDATE\n",
            "=\"100034\",\"18\",\"One Direction\",\"User Content\",\"MIDI\",\"User Update\"\n",
            "=\"100056\",\"A Parallel World\",\"BINI\",\"User Content\",\"MIDI\",\"User Update\"\n",
        );
        let (songs, report) = parse(text, Order::Auto);
        assert_eq!(songs.len(), 2);
        assert_eq!(report.rejected(), 0);
        assert_eq!(songs[0].num, 100034);
        assert_eq!(songs[0].title, "18");
        assert_eq!(songs[0].artist, "One Direction");
        assert_eq!(songs[1].artist, "BINI");
    }

    #[test]
    fn skips_bom_and_header_only_once() {
        let text = "\u{feff}number,title,artist\n1,A,B\nnumber,title,artist\n";
        let (songs, report) = parse(text, Order::Auto);
        assert_eq!(songs.len(), 1);
        // the second caption line is a real rejection, not a second header
        assert_eq!(report.rejected(), 1);
        assert_eq!(songs[0].title, "A");
    }

    #[test]
    fn detects_pipe_and_tab() {
        assert_eq!(detect_delimiter(&["1|title|artist"]), '|');
        assert_eq!(detect_delimiter(&["1\ttitle\tartist"]), '\t');
        assert_eq!(detect_delimiter(&["1;title;artist"]), ';');
        assert_eq!(detect_delimiter(&["1,title,artist"]), ',');
    }

    #[test]
    fn parses_three_columns() {
        let (songs, report) = parse("10001|Bohemian Rhapsody|Queen\n", Order::Auto);
        assert_eq!(songs.len(), 1);
        assert_eq!(songs[0].num, 10001);
        assert_eq!(songs[0].title, "Bohemian Rhapsody");
        assert_eq!(songs[0].artist, "Queen");
        assert_eq!(report.rejected(), 0);
    }

    #[test]
    fn parses_six_columns() {
        let (songs, _) = parse("1|Song|Artist|Pop|Karaoke|2024|", Order::Nta);
        assert_eq!(songs[0].genre, "Pop");
        assert_eq!(songs[0].stype, "Karaoke");
        assert_eq!(songs[0].release, "2024");
    }

    #[test]
    fn rejects_bad_rows() {
        let text = "1|Title|Artist\nonly-one-column\nabc|Title2|Artist2\n1|Dup|Artist3\n";
        let (songs, report) = parse(text, Order::Auto);
        assert_eq!(songs.len(), 1);
        assert_eq!(report.invalid.len(), 1);
        assert_eq!(report.bad_number.len(), 1);
        assert_eq!(report.duplicate.len(), 1);
    }

    #[test]
    fn empty_artist_becomes_unknown() {
        let (songs, _) = parse("1|Title|\n", Order::Nta);
        assert_eq!(songs[0].artist, DEFAULT_ARTIST);
    }

    #[test]
    fn semicolons_become_commas() {
        let (songs, _) = parse("1|Title|A;B\n", Order::Nta);
        assert_eq!(songs[0].artist, "A,B");
        let (songs, _) = parse("1|Title|A;B, C\n", Order::Nta);
        assert_eq!(songs[0].artist, "A;B, C");
    }

    #[test]
    fn user_content_is_skipped() {
        let (songs, report) = parse("1|Title|Artist|||user content\n", Order::Nta);
        assert!(songs.is_empty());
        assert_eq!(report.user_content, 1);
    }

    #[test]
    fn csv_with_quoted_commas() {
        let text = "\"10001\",\"Hello, Goodbye\",\"The Beatles\"\n";
        let (songs, _) = parse(text, Order::Auto);
        assert_eq!(songs[0].title, "Hello, Goodbye");
        assert_eq!(songs[0].artist, "The Beatles");
    }

    #[test]
    fn column_order_variants() {
        let (songs, _) = parse("Title|42|Artist\n", Order::Tna);
        assert_eq!(songs[0].num, 42);
        assert_eq!(songs[0].title, "Title");
        let (songs, _) = parse("Artist|Title|42\n", Order::Atn);
        assert_eq!(songs[0].num, 42);
        assert_eq!(songs[0].artist, "Artist");
    }

    #[test]
    fn auto_detects_title_first() {
        let (songs, _) = parse("Yesterday|7|Beatles\n", Order::Auto);
        assert_eq!(songs[0].num, 7);
    }
}
