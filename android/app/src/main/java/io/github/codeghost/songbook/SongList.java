package io.github.codeghost.songbook;

import java.util.ArrayList;
import java.util.HashSet;
import java.util.List;
import java.util.Locale;
import java.util.Set;

/**
 * Song list import: the 3/6 column format of MEGA Karaoke Songbook plus plain
 * CSV. Mirrors cli/src/import.rs (which mirrors the app's sbmanager.bas).
 */
public final class SongList {

    /** Artist used when the field is empty (app: main._v24_1). */
    public static final String DEFAULT_ARTIST = "(unknown)";

    public enum Order {
        AUTO, NTA, TNA, ATN;

        public static Order parse(String s) {
            if ("nta".equals(s)) return NTA;
            if ("tna".equals(s)) return TNA;
            if ("atn".equals(s)) return ATN;
            return AUTO;
        }

        public String id() {
            return name().toLowerCase(Locale.ROOT);
        }
    }

    public static final class Song {
        public final long num;
        public final String title;
        public final String artist;
        public final String genre;
        public final String stype;
        public final String release;
        public final int line;

        public Song(long num, String title, String artist, String genre, String stype, String release, int line) {
            this.num = num;
            this.title = title;
            this.artist = artist;
            this.genre = genre;
            this.stype = stype;
            this.release = release;
            this.line = line;
        }
    }

    public static final class Reject {
        public final String reason;
        public final int line;
        public final String raw;

        Reject(String reason, int line, String raw) {
            this.reason = reason;
            this.line = line;
            this.raw = raw;
        }
    }

    public static final class Report {
        public final List<Reject> rejects = new ArrayList<>();
        public int userContent;

        public int rejected() {
            return rejects.size();
        }

        /** Grouped summary in the wording of the app's import dialog. */
        public String summary() {
            StringBuilder b = new StringBuilder();
            String[] reasons = {"Invalid line", "No title", "Invalid number", "Number exists"};
            for (String reason : reasons) {
                int n = 0;
                for (Reject r : rejects) {
                    if (r.reason.equals(reason)) n++;
                }
                if (n == 0) continue;
                if (b.length() > 0) b.append('\n');
                b.append(reason).append(" (").append(n).append("):\n");
                int shown = 0;
                for (Reject r : rejects) {
                    if (!r.reason.equals(reason)) continue;
                    if (shown == 2) {
                        b.append("  ... and ").append(n - shown).append(" more");
                        break;
                    }
                    b.append("  line ").append(r.line).append(": ").append(truncate(r.raw, 60)).append('\n');
                    shown++;
                }
            }
            return b.toString().trim();
        }
    }

    private SongList() {}

    private static String truncate(String s, int max) {
        return s.length() <= max ? s : s.substring(0, max) + "\u2026";
    }

    /** Pick the delimiter that yields the most 3+ column lines. */
    public static char detectDelimiter(List<String> lines) {
        List<String> sample = new ArrayList<>();
        for (String l : lines) {
            if (!l.trim().isEmpty()) {
                sample.add(l);
                if (sample.size() == 50) break;
            }
        }
        if (sample.isEmpty()) return '|';
        char[] candidates = {'|', '\t', ';', ','};
        char best = '|';
        int bestScore = 0;
        for (char d : candidates) {
            int min = Integer.MAX_VALUE;
            int sum = 0;
            for (String l : sample) {
                int n = splitFields(l, d).size();
                min = Math.min(min, n);
                sum += n;
            }
            if (min < 2 || sum < sample.size() * 3) continue;
            int score = 0;
            for (String l : sample) {
                if (splitFields(l, d).size() >= 3) score++;
            }
            if (score > bestScore) {
                best = d;
                bestScore = score;
            }
        }
        return best;
    }

    /** Split one line; quoted values are honoured for ',' and ';' (RFC 4180). */
    public static List<String> splitFields(String line, char delim) {
        List<String> fields = new ArrayList<>();
        if (delim == ',' || delim == ';') {
            StringBuilder cur = new StringBuilder();
            boolean inQuotes = false;
            for (int i = 0; i < line.length(); i++) {
                char c = line.charAt(i);
                if (c == '"' && inQuotes && i + 1 < line.length() && line.charAt(i + 1) == '"') {
                    cur.append('"');
                    i++;
                } else if (c == '"') {
                    inQuotes = !inQuotes;
                } else if (c == delim && !inQuotes) {
                    fields.add(cur.toString().trim());
                    cur.setLength(0);
                } else {
                    cur.append(c);
                }
            }
            fields.add(cur.toString().trim());
        } else {
            int start = 0;
            for (int i = 0; i <= line.length(); i++) {
                if (i == line.length() || line.charAt(i) == delim) {
                    fields.add(stripQuotes(line.substring(start, i).trim()));
                    start = i + 1;
                }
            }
        }
        return fields;
    }

    private static String stripQuotes(String s) {
        if (s.length() >= 2 && s.charAt(0) == '"' && s.charAt(s.length() - 1) == '"') {
            return s.substring(1, s.length() - 1);
        }
        return s;
    }

    private static boolean looksNumeric(String s) {
        try {
            Double.parseDouble(s);
            return true;
        } catch (NumberFormatException e) {
            return false;
        }
    }

    public static void parse(String text, Order order, List<Song> songs, Report report) {
        String[] lines = text.split("\r?\n", -1);
        List<String> all = new ArrayList<>(lines.length);
        for (String l : lines) all.add(l);
        char delim = detectDelimiter(all);
        Set<Long> seen = new HashSet<>();

        for (int idx = 0; idx < lines.length; idx++) {
            String raw = lines[idx];
            if (raw.trim().isEmpty()) continue;
            int lineNo = idx + 1;
            List<String> cols = splitFields(raw, delim);
            if (cols.size() < 3) {
                report.rejects.add(new Reject("Invalid line", lineNo, raw));
                continue;
            }
            String c0 = cols.get(0);
            String c1 = cols.get(1);
            String c2 = cols.get(2);
            if (c0.isEmpty() || c1.isEmpty()) {
                report.rejects.add(new Reject("Invalid line", lineNo, raw));
                continue;
            }
            if (c2.isEmpty()) c2 = DEFAULT_ARTIST;

            Order o = order;
            if (o == Order.AUTO) {
                if (looksNumeric(c0)) o = Order.NTA;
                else if (looksNumeric(c1)) o = Order.TNA;
                else if (looksNumeric(c2)) o = Order.ATN;
                else o = Order.NTA;
            }
            String numS;
            String titleS;
            String artistS;
            if (o == Order.TNA) {
                numS = c1;
                titleS = c0;
                artistS = c2;
            } else if (o == Order.ATN) {
                numS = c2;
                titleS = c1;
                artistS = c0;
            } else {
                numS = c0;
                titleS = c1;
                artistS = c2;
            }

            long num;
            try {
                double v = Double.parseDouble(numS);
                if (Double.isInfinite(v) || Double.isNaN(v)) throw new NumberFormatException();
                num = (long) v;
            } catch (NumberFormatException e) {
                report.rejects.add(new Reject("Invalid number", lineNo, raw));
                continue;
            }
            if (titleS.isEmpty()) {
                report.rejects.add(new Reject("No title", lineNo, raw));
                continue;
            }
            if (!seen.add(num)) {
                report.rejects.add(new Reject("Number exists", lineNo, raw));
                continue;
            }
            String release = cols.size() > 5 ? cols.get(5) : "";
            if ("user content".equals(release)) {
                report.userContent++;
                continue;
            }
            String artist = artistS;
            if (artist.indexOf(',') < 0) {
                artist = artist.replace(';', ',');
            }
            songs.add(new Song(
                    num,
                    titleS,
                    artist,
                    cols.size() > 3 ? cols.get(3) : "",
                    cols.size() > 4 ? cols.get(4) : "",
                    release,
                    lineNo));
        }
    }
}
