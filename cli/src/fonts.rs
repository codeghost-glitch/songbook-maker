//! Helvetica / Helvetica-Bold metrics (Adobe AFM widths, 1/1000 em).
//!
//! Only the standard-14 Type1 fonts are used, so nothing is embedded in the
//! PDF and no font files are needed at build or run time.

/// Widths for ASCII 32..=126 of Helvetica.
const HELVETICA: [u16; 95] = [
    278, 278, 355, 556, 556, 889, 667, 191, 333, 333, 389, 584, 278, 333, 278, 278, // 32..47
    556, 556, 556, 556, 556, 556, 556, 556, 556, 556, // digits
    278, 278, 584, 584, 584, 556, 1015, // :;<=>?@
    667, 667, 722, 722, 667, 611, 778, 722, 278, 500, 667, 556, 833, 722, 778, 667, // A..P
    778, 722, 667, 611, 722, 667, 944, 667, 667, 611, // Q..Z
    278, 278, 278, 469, 556, 333, // [\\]^_`
    556, 556, 500, 556, 556, 278, 556, 556, 222, 222, 500, 222, 833, 556, 556, 556, // a..p
    556, 333, 500, 278, 556, 500, 722, 500, 500, 500, // q..z
    334, 260, 334, 584,
];

/// Widths for ASCII 32..=126 of Helvetica-Bold.
const HELVETICA_BOLD: [u16; 95] = [
    278, 333, 474, 556, 556, 889, 722, 238, 333, 333, 389, 584, 278, 333, 278, 278,
    556, 556, 556, 556, 556, 556, 556, 556, 556, 556,
    333, 333, 584, 584, 584, 611, 975,
    722, 722, 722, 722, 667, 611, 778, 722, 278, 556, 722, 611, 833, 722, 778, 667,
    778, 722, 667, 611, 722, 667, 944, 667, 667, 611,
    333, 278, 333, 584, 556, 333,
    556, 611, 556, 611, 556, 333, 611, 611, 278, 278, 556, 278, 889, 611, 611, 611,
    611, 389, 556, 333, 611, 556, 778, 556, 556, 500,
    389, 280, 389, 584,
];

/// Glyph advance in 1/1000 em.
pub fn char_width(ch: char, bold: bool) -> u16 {
    let table = if bold { &HELVETICA_BOLD } else { &HELVETICA };
    let code = ch as u32;
    if (32..=126).contains(&code) {
        table[(code - 32) as usize]
    } else {
        match code {
            0x2018 | 0x2019 => 222,      // curly single quotes
            0x201c | 0x201d => 333,      // curly double quotes
            0x2013 => 556,               // en dash
            0x2014 => 1000,              // em dash
            _ => 556,                    // sensible fallback
        }
    }
}

/// Width of `text` at `size` points.
pub fn text_width(text: &str, size: f32, bold: bool) -> f32 {
    let units: u32 = text.chars().map(|c| char_width(c, bold) as u32).sum();
    units as f32 * size / 1000.0
}

/// Approximate line height (ascent + descent) used for vertical centring.
pub fn text_height(size: f32) -> f32 {
    size * 1.15
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn widths_are_monotonic_in_size() {
        assert!(text_width("Hello", 8.0, false) < text_width("Hello", 12.0, false));
    }

    #[test]
    fn bold_is_wider_than_regular() {
        assert!(text_width("WALTZ", 10.0, true) > text_width("WALTZ", 10.0, false));
    }

    #[test]
    fn known_helvetica_advance() {
        // "W" is 944/1000 em in Helvetica
        assert!((text_width("W", 1000.0, false) - 944.0).abs() < 0.01);
    }
}
