package io.github.codeghost.songbook;

import java.util.ArrayList;
import java.util.Collections;
import java.util.Comparator;
import java.util.List;
import java.util.Locale;

/**
 * Songbook layout and drawing: a 1:1 port of the print engine in MEGA Karaoke
 * Songbook 2.59 (pprint.bas), matching cli/src/render.rs.
 *
 * <p>Geometry in PDF points (72 per inch): row height 19, column gap 8, number
 * column 40 (42 when numbers exceed 9 999 999), title 65% and artist 35% of the
 * rest, 35 pt top band, 20 pt bottom band.
 */
public final class Sheet {

    public static final float ROW_H = 19f;
    public static final float GAP = 8f;
    public static final float NUM_W = 40f;
    public static final float TOP_BAND = 35f;
    public static final float BOT_BAND = 20f;
    public static final float LOGO_W = 119f;

    public static final float[] MARGINS = {0.20f, 0.25f, 0.30f, 0.40f, 0.50f};

    public enum Paper {
        A4(8.3f, 11.7f, "A4"),
        LETTER(8.5f, 11f, "Letter"),
        LONG(8.5f, 13f, "Long"),
        LEGAL(8.5f, 14f, "Legal");

        public final float wIn;
        public final float hIn;
        public final String label;

        Paper(float wIn, float hIn, String label) {
            this.wIn = wIn;
            this.hIn = hIn;
            this.label = label;
        }
    }

    /** rows per page, index [paper][margin] - hard coded in the app. */
    private static final int[][] ROWS = {
        {39, 39, 39, 38, 37},
        {37, 36, 36, 35, 35},
        {44, 44, 44, 43, 42},
        {48, 48, 47, 47, 46},
    };

    /** y of the first row, index [paper][margin]. */
    private static final float[][] TOP = {
        {58f, 58f, 58f, 68f, 77f},
        {52f, 62f, 62f, 72f, 71f},
        {57f, 58f, 58f, 67f, 77f},
        {56f, 56f, 66f, 65f, 75f},
    };

    public enum Sections {
        CONTINUOUS("Continuous"),
        NEWPAGE("New page per letter"),
        NONE("No letters");

        public final String label;

        Sections(String label) {
            this.label = label;
        }
    }

    public enum SortKey {
        TITLE("Title"),
        ARTIST("Artist"),
        NUMBER("Number");

        public final String label;

        SortKey(String label) {
            this.label = label;
        }
    }

    public static final class Options {
        public Paper paper = Paper.A4;
        public float margin = 0.25f;
        public int columns = 3;
        public SongList.Order order = SongList.Order.AUTO;
        public SortKey sort = SortKey.TITLE;
        public Sections sections = Sections.CONTINUOUS;
        public boolean noArtist;
        public String name = "";
        public String headerLeft = "";
        public String headerCenter = "";
        public String headerRight = "";
        public String footerCenter = "";
        public boolean pageNumbers = true;
        public boolean mirror;
        public boolean bold = true;
        public float scale = 1f;
        public float numSize = 7.5f;
        public float titleSize = 8f;
        public float artistSize = 6.5f;
        public float letterSize = 11f;
        public float headerSize = 9f;
        public float footerSize = 6.5f;
        public int pageColor = PdfDoc.WHITE;
        public int lineColor = 0xFFBDBDBD;
        public int bandColor = PdfDoc.BLACK;
        public int bandOpacity = 34;
        public int numColor = PdfDoc.BLACK;
        public int titleColor = PdfDoc.BLACK;
        public int artistColor = PdfDoc.BLACK;
        public int sectionBg = 0xFFEEEEEE;
        public int sectionColor = PdfDoc.BLACK;
        public int headerColor = PdfDoc.BLACK;
        public int footerColor = PdfDoc.BLACK;
        public Integer numBg;
        public Integer titleBg;
        public Integer artistBg;
        /** optional logo: ARGB pixels, row major */
        public int[] logoPixels;
        public int logoW;
        public int logoH;
    }

    public static final class Result {
        public PdfDoc pdf;
        public int pages;
        public float pageW;
        public float pageH;
        public int columns;
        public int rows;
        public float[] cells;
    }

    private static final class Item {
        final boolean letter;
        final char ch;
        final int song;

        Item(char ch) {
            this.letter = true;
            this.ch = ch;
            this.song = -1;
        }

        Item(int song) {
            this.letter = false;
            this.ch = 0;
            this.song = song;
        }
    }

    private Sheet() {}

    private static int marginIndex(float m) {
        for (int i = 0; i < MARGINS.length; i++) {
            if (Math.abs(MARGINS[i] - m) < 0.001f) return i;
        }
        return -1;
    }

    private static char sectionKey(SongList.Song s, SortKey sort) {
        String v = sort == SortKey.ARTIST ? s.artist : s.title;
        String t = v.trim();
        if (t.isEmpty()) return '#';
        char c = Character.toUpperCase(t.charAt(0));
        return Character.isLetter(c) ? c : '#';
    }

    /** A-Z first, then '#' for titles that start with a digit or symbol. */
    private static int sectionRank(char c) {
        return c == '#' ? 1 : 0;
    }

    private static final class Fitted {
        final String text;
        final float size;

        Fitted(String text, float size) {
            this.text = text;
            this.size = size;
        }
    }

    private static Fitted truncateToWidth(String text, float size, float maxw, boolean bold) {
        float sz = size;
        float floor = Math.max(size * 0.45f, 2.6f);
        while (Fonts.width(text, sz, bold) > maxw && sz > floor) {
            sz -= 0.25f;
        }
        String s = text;
        while (!s.isEmpty() && Fonts.width(s, sz, bold) > maxw) {
            s = s.substring(0, s.length() - 1);
        }
        return new Fitted(s, sz);
    }

    public static Result render(List<SongList.Song> songs, Options opt) {
        int midx = marginIndex(opt.margin);
        if (midx < 0) throw new IllegalArgumentException("margin must be 0.20 0.25 0.30 0.40 0.50");
        if (opt.columns < 2 || opt.columns > 4) throw new IllegalArgumentException("columns must be 2, 3 or 4");
        if (songs.isEmpty()) throw new IllegalArgumentException("no songs to print");

        int pi = opt.paper.ordinal();
        float pageW = (float) Math.floor(opt.paper.wIn * 72f);
        float pageH = (float) Math.floor(opt.paper.hIn * 72f);
        float m = (float) Math.floor(opt.margin * 72f);
        int rows = ROWS[pi][midx];
        float top = TOP[pi][midx];
        float bottom = top + rows * ROW_H;

        int cols = opt.columns;
        float colW = (pageW - 2 * m - (cols - 1) * GAP) / cols;
        float[] colX = new float[cols];
        float x = m;
        for (int i = 0; i < cols; i++) {
            colX[i] = Math.round(x);
            x += colW + GAP;
        }

        float numW = NUM_W;
        float rest = (float) Math.floor(colW - NUM_W);
        float titleW = (float) Math.floor(0.65f * rest);
        float artistW = (float) Math.floor(0.35f * rest);
        long maxNum = 0;
        for (SongList.Song s : songs) maxNum = Math.max(maxNum, s.num);
        if (maxNum > 9_999_999L) {
            numW += 2f;
            titleW -= 2f;
        }
        float[] cells;
        if (opt.noArtist) {
            cells = opt.order == SongList.Order.TNA
                    ? new float[]{titleW + artistW, numW, 0f}
                    : new float[]{numW, titleW + artistW, 0f};
        } else if (opt.order == SongList.Order.TNA) {
            cells = new float[]{titleW, numW, artistW};
        } else {
            cells = new float[]{numW, titleW, artistW};
        }
        float totalW = cells[0] + cells[1] + cells[2];

        float scale = Math.max(opt.scale, 0.1f);
        float szNum = opt.numSize / scale;
        float szTitle = opt.titleSize / scale;
        float szArtist = opt.artistSize / scale;
        float szLetter = opt.letterSize / scale;
        float szHeader = opt.headerSize / scale;
        float szFooter = opt.footerSize / scale;

        // ------------------------------------------------ ordering
        List<Integer> orderIdx = new ArrayList<>(songs.size());
        for (int i = 0; i < songs.size(); i++) orderIdx.add(i);
        Comparator<Integer> cmp;
        if (opt.sort == SortKey.ARTIST) {
            cmp = Comparator.comparing((Integer i) -> lower(songs.get(i).artist))
                    .thenComparing(i -> lower(songs.get(i).title))
                    .thenComparing(i -> songs.get(i).num);
        } else if (opt.sort == SortKey.NUMBER) {
            cmp = Comparator.comparing((Integer i) -> songs.get(i).num)
                    .thenComparing(i -> lower(songs.get(i).title));
        } else {
            cmp = Comparator.comparing((Integer i) -> lower(songs.get(i).title))
                    .thenComparing(i -> lower(songs.get(i).artist))
                    .thenComparing(i -> songs.get(i).num);
        }
        Collections.sort(orderIdx, cmp);
        if (opt.sections == Sections.CONTINUOUS) {
            Collections.sort(orderIdx, new Comparator<Integer>() {
                @Override
                public int compare(Integer a, Integer b) {
                    int ra = sectionRank(sectionKey(songs.get(a), opt.sort));
                    int rb = sectionRank(sectionKey(songs.get(b), opt.sort));
                    return Integer.compare(ra, rb);
                }
            });
        }

        List<Item> items = new ArrayList<>();
        if (opt.sections == Sections.NONE) {
            for (int i : orderIdx) items.add(new Item(i));
        } else {
            Character last = null;
            for (int i : orderIdx) {
                char k = sectionKey(songs.get(i), opt.sort);
                if (last == null || last != k) {
                    items.add(new Item(k));
                    last = k;
                }
                items.add(new Item(i));
            }
        }

        int perPage = rows * cols;
        List<List<Item>> pages = new ArrayList<>();
        if (opt.sections == Sections.NEWPAGE) {
            List<Item> cur = new ArrayList<>();
            for (Item it : items) {
                if (it.letter) {
                    while (!cur.isEmpty()) {
                        List<Item> page = new ArrayList<>();
                        int take = Math.min(cur.size(), perPage);
                        for (int i = 0; i < take; i++) page.add(cur.remove(0));
                        pages.add(page);
                    }
                    cur.add(it);
                } else {
                    cur.add(it);
                    if (cur.size() >= perPage) {
                        pages.add(cur);
                        cur = new ArrayList<>();
                    }
                }
            }
            while (!cur.isEmpty()) {
                List<Item> page = new ArrayList<>();
                int take = Math.min(cur.size(), perPage);
                for (int i = 0; i < take; i++) page.add(cur.remove(0));
                pages.add(page);
            }
        } else {
            List<Item> cur = new ArrayList<>();
            for (Item it : items) {
                cur.add(it);
                if (cur.size() == perPage) {
                    pages.add(cur);
                    cur = new ArrayList<>();
                }
            }
            if (!cur.isEmpty() || pages.isEmpty()) pages.add(cur);
        }
        if (pages.isEmpty()) pages.add(new ArrayList<Item>());

        // ------------------------------------------------ drawing
        PdfDoc pdf = new PdfDoc();
        int logoIndex = -1;
        if (opt.logoPixels != null && opt.logoW > 0 && opt.logoH > 0) {
            byte[] rgb = new byte[opt.logoW * opt.logoH * 3];
            for (int i = 0; i < opt.logoPixels.length && i < opt.logoW * opt.logoH; i++) {
                int p = opt.logoPixels[i];
                rgb[i * 3] = (byte) ((p >> 16) & 0xFF);
                rgb[i * 3 + 1] = (byte) ((p >> 8) & 0xFF);
                rgb[i * 3 + 2] = (byte) (p & 0xFF);
            }
            logoIndex = pdf.addImage("Im1", opt.logoW, opt.logoH, rgb);
        }
        int bandTop = PdfDoc.blend(opt.bandColor, opt.pageColor, opt.bandOpacity / 255f);

        int totalPages = pages.size();
        for (int p = 0; p < totalPages; p++) {
            int pno = p + 1;
            List<Item> pageItems = pages.get(p);
            pdf.page(pageW, pageH);
            pdf.fillRect(0, 0, pageW, pageH, opt.pageColor);

            for (int c = 0; c < cols; c++) {
                for (int ci = 0; ci < 3; ci++) {
                    Integer bg = ci == 0 ? opt.numBg : ci == 1 ? opt.titleBg : opt.artistBg;
                    if (bg == null || cells[ci] <= 0) continue;
                    float cw = ci == 0 ? 0 : ci == 1 ? cells[0] : cells[0] + cells[1];
                    pdf.fillRect(colX[c] + cw, top, cells[ci], bottom - top, bg);
                }
            }

            List<Item> flow = new ArrayList<>();
            if (opt.sections == Sections.NEWPAGE) {
                for (Item it : pageItems) if (!it.letter) flow.add(it);
            } else {
                flow.addAll(pageItems);
            }
            int pos = 0;
            boolean done = false;
            for (int c = 0; c < cols && !done; c++) {
                for (int r = 0; r < rows; r++) {
                    if (pos >= flow.size()) {
                        done = true;
                        break;
                    }
                    Item it = flow.get(pos++);
                    float yTop = top + r * ROW_H;
                    if (it.letter) {
                        String txt = String.valueOf(it.ch);
                        pdf.fillRect(colX[c] - 1, yTop - 1, totalW - 2, ROW_H + 2, opt.sectionBg);
                        pdf.strokeRect(colX[c] - 1, yTop - 1, totalW - 2, ROW_H + 2, PdfDoc.BLACK, 0.2f);
                        Fitted f = truncateToWidth(txt, szLetter, totalW - 2, true);
                        pdf.text(f.text, colX[c] - 1 + (totalW - 2) / 2, yTop + ROW_H / 2 + 0.45f * f.size,
                                f.size, opt.sectionColor, true, PdfDoc.ALIGN_CENTER);
                    } else {
                        SongList.Song song = songs.get(it.song);
                        float cx = colX[c];
                        for (int ci = 0; ci < 3; ci++) {
                            if (cells[ci] <= 0) continue;
                            pdf.strokeRect(cx, yTop, cells[ci], ROW_H, opt.lineColor, 0.2f);
                            cx += cells[ci];
                        }
                        float yBot = yTop + ROW_H;
                        Fitted n = truncateToWidth(String.valueOf(song.num), szNum, cells[0] - 1, opt.bold);
                        if (opt.order == SongList.Order.TNA) {
                            drawTextCell(pdf, song.title, colX[c] + cells[0], cells[1], yBot, szTitle, opt.titleColor, opt.bold);
                            pdf.text(n.text, colX[c] + cells[0] / 2, yTop + ROW_H / 2 + 0.45f * n.size,
                                    n.size, opt.numColor, opt.bold, PdfDoc.ALIGN_CENTER);
                        } else {
                            pdf.text(n.text, colX[c] + cells[0] / 2, yTop + ROW_H / 2 + 0.45f * n.size,
                                    n.size, opt.numColor, opt.bold, PdfDoc.ALIGN_CENTER);
                            drawTextCell(pdf, song.title, colX[c] + cells[0], cells[1], yBot, szTitle, opt.titleColor, opt.bold);
                        }
                        if (!opt.noArtist) {
                            drawTextCell(pdf, song.artist, colX[c] + cells[0] + cells[1], cells[2], yBot,
                                    szArtist, opt.artistColor, opt.bold);
                        }
                    }
                }
            }

            // header and footer
            pdf.fillRect(m, m, pageW - 2 * m, TOP_BAND, bandTop);
            pdf.fillRect(m, pageH - m - BOT_BAND, pageW - 2 * m, BOT_BAND, bandTop);

            float baseTop = m + 17.5f + Fonts.height(szHeader) / 2f - 1f;
            float baseBot = pageH - m - 11f + Fonts.height(szFooter) / 2f;
            float maxw = pageW - 2 * m - 10f;

            boolean mirror = opt.mirror && pno % 2 == 0;
            String leftTxt = mirror ? opt.headerRight : opt.headerLeft;
            String rightTxt = mirror ? opt.headerLeft : opt.headerRight;

            Character pageLetter = null;
            if (opt.sections == Sections.NEWPAGE) {
                for (Item it : pageItems) {
                    if (it.letter) {
                        pageLetter = it.ch;
                        break;
                    }
                }
            }
            if (pageLetter != null) {
                Fitted f = truncateToWidth(String.valueOf(pageLetter), szHeader * 1.4f, maxw / 3f, true);
                pdf.text(f.text, m + 5, baseTop, f.size, opt.headerColor, true, PdfDoc.ALIGN_LEFT);
            } else if (!leftTxt.isEmpty()) {
                Fitted f = truncateToWidth(leftTxt, szHeader, maxw, opt.bold);
                pdf.text(f.text, m + 5, baseTop, f.size, opt.headerColor, opt.bold, PdfDoc.ALIGN_LEFT);
            }
            if (!rightTxt.isEmpty()) {
                Fitted f = truncateToWidth(rightTxt, szHeader, maxw, opt.bold);
                pdf.text(f.text, pageW - m - 5, baseTop, f.size, opt.headerColor, opt.bold, PdfDoc.ALIGN_RIGHT);
            }
            String center = opt.headerCenter.isEmpty() ? opt.name : opt.headerCenter;
            if (!center.isEmpty()) {
                Fitted f = truncateToWidth(center, szHeader, maxw, opt.bold);
                pdf.text(f.text, pageW / 2, baseTop, f.size, opt.headerColor, opt.bold, PdfDoc.ALIGN_CENTER);
            }
            if (!opt.footerCenter.isEmpty()) {
                Fitted f = truncateToWidth(opt.footerCenter, szFooter, maxw, opt.bold);
                pdf.text(f.text, pageW / 2, baseBot, f.size, opt.footerColor, opt.bold, PdfDoc.ALIGN_CENTER);
            }
            if (opt.pageNumbers) {
                String s = pno + " / " + totalPages;
                if (mirror) {
                    pdf.text(s, m + 5, baseBot, szFooter, opt.footerColor, opt.bold, PdfDoc.ALIGN_LEFT);
                } else {
                    pdf.text(s, pageW - m - 5, baseBot, szFooter, opt.footerColor, opt.bold, PdfDoc.ALIGN_RIGHT);
                }
            }

            if (logoIndex >= 0) {
                float ly = pageH - m - BOT_BAND;
                float lx = mirror ? pageW - m - 5 - LOGO_W : m + 5;
                pdf.image(logoIndex, lx, ly, LOGO_W, BOT_BAND);
            }
        }

        Result result = new Result();
        result.pdf = pdf;
        result.pages = totalPages;
        result.pageW = pageW;
        result.pageH = pageH;
        result.columns = cols;
        result.rows = rows;
        result.cells = cells;
        return result;
    }

    private static String lower(String s) {
        return s.toLowerCase(Locale.ROOT);
    }

    /** Shrink to fit, then a two line fallback - same rule as the app. */
    private static void drawTextCell(PdfDoc pdf, String text, float x, float cellW, float yBot,
                                     float size, int color, boolean bold) {
        if (text == null || text.isEmpty()) return;
        float maxw = cellW - 3f;
        float floor = Math.max(size * 0.45f, 2.6f);
        float sz = size;
        while (Fonts.width(text, sz, bold) > maxw && sz > floor) {
            sz -= 0.25f;
        }
        if (Fonts.width(text, sz, bold) <= maxw) {
            Fitted f = truncateToWidth(text, sz, maxw, bold);
            pdf.text(f.text, x + 2, yBot - ROW_H / 2 + 0.45f * f.size, f.size, color, bold, PdfDoc.ALIGN_LEFT);
            return;
        }
        int best = -1;
        StringBuilder acc = new StringBuilder();
        for (int i = 0; i < text.length(); i++) {
            char ch = text.charAt(i);
            acc.append(ch);
            if (ch == ' ' || ch == '-') best = i;
            if (Fonts.width(acc.toString(), sz, bold) > maxw) break;
        }
        String line1;
        String line2;
        if (best >= 0) {
            line1 = text.substring(0, best + 1);
            line2 = text.substring(best + 1);
        } else {
            int cut = acc.length();
            line1 = text.substring(0, cut);
            line2 = text.substring(cut);
        }
        if (line2.trim().isEmpty()) {
            Fitted f = truncateToWidth(text, sz, maxw, bold);
            pdf.text(f.text, x + 2, yBot - ROW_H / 2 + 0.45f * f.size, f.size, color, bold, PdfDoc.ALIGN_LEFT);
        } else {
            Fitted l1 = truncateToWidth(line1.trim(), sz, maxw, bold);
            Fitted l2 = truncateToWidth(line2.trim(), sz, maxw, bold);
            pdf.text(l1.text, x + 2, yBot - ROW_H * 0.58f, l1.size, color, bold, PdfDoc.ALIGN_LEFT);
            pdf.text(l2.text, x + 2, yBot - ROW_H * 0.16f, l2.size, color, bold, PdfDoc.ALIGN_LEFT);
        }
    }
}
