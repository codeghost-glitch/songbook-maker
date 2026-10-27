package io.github.codeghost.songbook;

/** Helvetica / Helvetica-Bold metrics (Adobe AFM widths, 1/1000 em). */
public final class Fonts {

    private static final short[] HELV = {
        278, 278, 355, 556, 556, 889, 667, 191, 333, 333, 389, 584, 278, 333, 278, 278,
        556, 556, 556, 556, 556, 556, 556, 556, 556, 556,
        278, 278, 584, 584, 584, 556, 1015,
        667, 667, 722, 722, 667, 611, 778, 722, 278, 500, 667, 556, 833, 722, 778, 667,
        778, 722, 667, 611, 722, 667, 944, 667, 667, 611,
        278, 278, 278, 469, 556, 333,
        556, 556, 500, 556, 556, 278, 556, 556, 222, 222, 500, 222, 833, 556, 556, 556,
        556, 333, 500, 278, 556, 500, 722, 500, 500, 500,
        334, 260, 334, 584
    };

    private static final short[] HELV_BOLD = {
        278, 333, 474, 556, 556, 889, 722, 238, 333, 333, 389, 584, 278, 333, 278, 278,
        556, 556, 556, 556, 556, 556, 556, 556, 556, 556,
        333, 333, 584, 584, 584, 611, 975,
        722, 722, 722, 722, 667, 611, 778, 722, 278, 556, 722, 611, 833, 722, 778, 667,
        778, 722, 667, 611, 722, 667, 944, 667, 667, 611,
        333, 278, 333, 584, 556, 333,
        556, 611, 556, 611, 556, 333, 611, 611, 278, 278, 556, 278, 889, 611, 611, 611,
        611, 389, 556, 333, 611, 556, 778, 556, 556, 500,
        389, 280, 389, 584
    };

    private Fonts() {}

    public static int charWidth(char c, boolean bold) {
        short[] table = bold ? HELV_BOLD : HELV;
        if (c >= 32 && c <= 126) {
            return table[c - 32];
        }
        switch (c) {
            case '\u2018':
            case '\u2019':
                return 222;
            case '\u201c':
            case '\u201d':
                return 333;
            case '\u2013':
                return 556;
            case '\u2014':
                return 1000;
            default:
                return 556;
        }
    }

    /** Width of the string at the given size in points. */
    public static float width(String s, float size, boolean bold) {
        int units = 0;
        for (int i = 0; i < s.length(); i++) {
            units += charWidth(s.charAt(i), bold);
        }
        return units * size / 1000f;
    }

    public static float height(float size) {
        return size * 1.15f;
    }
}
