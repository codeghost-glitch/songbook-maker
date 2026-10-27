#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
songbook.py - CSV/TXT -> songbook PDF.
A dependency-free reference implementation of the print engine that the Rust
version in cli/ ports (see docs/usage.md).

Import format:
    number | title | artist
    number | title | artist | category | song type | update
Delimiters: pipe ( | ), TAB, semicolon, comma.

Examples
--------
    python3 songbook.py songs.csv -o songbook.pdf
    python3 songbook.py songs.txt --paper letter --margin 0.3 --columns 2 \
            --order tna --sections newpage --name "Rey's Karaoke" -o out.pdf
"""
import sys, os, re, zlib, argparse, csv, io

# --------------------------------------------------------------------------
# 1. Font metrics (Helvetica / Helvetica-Bold, Adobe AFM widths /1000 em)
# --------------------------------------------------------------------------
_HELV = {
 32:278,33:278,34:355,35:556,36:556,37:889,38:667,39:191,40:333,41:333,42:389,
 43:584,44:278,45:333,46:278,47:278,48:556,49:556,50:556,51:556,52:556,53:556,
 54:556,55:556,56:556,57:556,58:278,59:278,60:584,61:584,62:584,63:556,64:1015,
 65:667,66:667,67:722,68:722,69:667,70:611,71:778,72:722,73:278,74:500,75:667,
 76:556,77:833,78:722,79:778,80:667,81:778,82:722,83:667,84:611,85:722,86:667,
 87:944,88:667,89:667,90:611,91:278,92:278,93:278,94:469,95:556,96:333,97:556,
 98:556,99:500,100:556,101:556,102:278,103:556,104:556,105:222,106:222,107:500,
 108:222,109:833,110:556,111:556,112:556,113:556,114:333,115:500,116:278,117:556,
 118:500,119:722,120:500,121:500,122:500,123:334,124:260,125:334,126:584}
_HELV_B = {
 32:278,33:333,34:474,35:556,36:556,37:889,38:722,39:238,40:333,41:333,42:389,
 43:584,44:278,45:333,46:278,47:278,48:556,49:556,50:556,51:556,52:556,53:556,
 54:556,55:556,56:556,57:556,58:333,59:333,60:584,61:584,62:584,63:611,64:975,
 65:722,66:722,67:722,68:722,69:667,70:611,71:778,72:722,73:278,74:556,75:722,
 76:611,77:833,78:722,79:778,80:667,81:778,82:722,83:667,84:611,85:722,86:667,
 87:944,88:667,89:667,90:611,91:333,92:278,93:333,94:584,95:556,96:333,97:556,
 98:611,99:556,100:611,101:556,102:333,103:611,104:611,105:278,106:278,107:556,
 108:278,109:889,110:611,111:611,112:611,113:611,114:389,115:556,116:333,117:611,
 118:556,119:778,120:556,121:556,122:500,123:389,124:280,125:389,126:584}

def char_pdf_width(ch, bold):
    o = ord(ch)
    if 32 <= o <= 126:
        return (_HELV_B if bold else _HELV)[o]
    if o in (0x2018, 0x2019):   # curly quotes
        return 222 if not bold else 278
    if o == 0x201C or o == 0x201D:
        return 333 if not bold else 500
    if o == 0x2013:              # en dash
        return 556
    if o == 0x2014:              # em dash
        return 1000
    return 556

def twidth(s, size, bold=False):
    """Text width in points, as Canvas.MeasureStringWidth would report."""
    return sum(char_pdf_width(c, bold) for c in s) * size / 1000.0

def theight(size):
    """Approximate text height (ascent+descent) used for vertical centring."""
    return size * 1.15

# --------------------------------------------------------------------------
# 2. Minimal PDF writer (no third-party modules)
# --------------------------------------------------------------------------
def _num(v):
    return ("%.2f" % v).rstrip("0").rstrip(".") if isinstance(v, float) else str(v)

def _esc(s):
    out = []
    for b in s.encode("cp1252", "replace"):
        c = chr(b)
        if c in "\\()":
            out.append("\\" + c)
        elif b < 32 or b > 126:
            out.append("\\%03o" % b)
        else:
            out.append(c)
    return "".join(out)

class PDF:
    def __init__(self):
        self.pages = []          # [{'w','h','ops':[...]}]
        self.images = {}         # name -> (w, h, rgbbytes)

    # ---- drawing primitives (y grows downwards, like the app's canvas) ----
    def page(self, w, h):
        self.pages.append({"w": w, "h": h, "ops": []})
        return self.pages[-1]

    def _op(self, s):
        self.pages[-1]["ops"].append(s)

    def _y(self, y):
        return self.pages[-1]["h"] - y

    def fill_rect(self, x, y, w, h, color):
        r, g, b = color
        self._op("%s %s %s rg %s %s %s %s re f" % (
            _num(r), _num(g), _num(b), _num(x), _num(self._y(y + h)),
            _num(w), _num(h)))

    def stroke_rect(self, x, y, w, h, color, lw=0.2):
        r, g, b = color
        self._op("%s %s %s RG %s w %s %s %s %s re S" % (
            _num(r), _num(g), _num(b), _num(lw), _num(x),
            _num(self._y(y + h)), _num(w), _num(h)))

    def line(self, x1, y1, x2, y2, color, lw=0.2):
        r, g, b = color
        self._op("%s %s %s RG %s w %s %s m %s %s l S" % (
            _num(r), _num(g), _num(b), _num(lw), _num(x1), _num(self._y(y1)),
            _num(x2), _num(self._y(y2))))

    def text(self, s, x, y_baseline, size, color, bold=False, align="left"):
        if not s:
            return
        r, g, b = color
        w = twidth(s, size, bold)
        if align == "center":
            x -= w / 2.0
        elif align == "right":
            x -= w
        self._op("BT /%s %s Tf %s %s %s rg 1 0 0 1 %s %s Tm (%s) Tj ET" % (
            "F2" if bold else "F1", _num(size), _num(r), _num(g), _num(b),
            _num(x), _num(self._y(y_baseline)), _esc(s)))

    def image(self, name, x, y, w, h):
        self._op("q %s 0 0 %s %s %s cm /%s Do Q" % (
            _num(w), _num(h), _num(x), _num(self._y(y + h)), name))

    # ---- serialisation ----
    def save(self, path, title="Songbook"):
        objs = {}
        n_pages = len(self.pages)
        font_ids = {"F1": 3, "F2": 4}
        img_ids = {}
        nxt = 5
        for nm in self.images:
            img_ids[nm] = nxt
            nxt += 1
        page_ids = []
        for _ in range(n_pages):
            page_ids.append(nxt)
            nxt += 1
        cont_ids = []
        for _ in range(n_pages):
            cont_ids.append(nxt)
            nxt += 1
        info_id = nxt

        objs[1] = "<< /Type /Catalog /Pages 2 0 R >>"
        objs[2] = "<< /Type /Pages /Count %d /Kids [%s] >>" % (
            n_pages, " ".join("%d 0 R" % i for i in page_ids))
        objs[3] = ("<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica "
                   "/Encoding /WinAnsiEncoding >>")
        objs[4] = ("<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica-Bold "
                   "/Encoding /WinAnsiEncoding >>")

        for i, (nm, (w, h, rgb)) in enumerate(self.images.items()):
            data = zlib.compress(rgb, 9)
            head = ("<< /Type /XObject /Subtype /Image /Width %d /Height %d "
                    "/ColorSpace /DeviceRGB /BitsPerComponent 8 "
                    "/Filter /FlateDecode /Length %d >>\nstream\n" % (w, h, len(data)))
            objs[img_ids[nm]] = ("RAW", head.encode("latin-1") + data + b"\nendstream")

        xobj = " ".join("/%s %d 0 R" % (nm, i) for nm, i in img_ids.items())
        for idx, pg in enumerate(self.pages):
            res = "<< /Font << /F1 3 0 R /F2 4 0 R >>"
            if xobj:
                res += " /XObject << %s >>" % xobj
            res += " >>"
            objs[page_ids[idx]] = (
                "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 %s %s] "
                "/Resources %s /Contents %d 0 R >>" % (
                    _num(pg["w"]), _num(pg["h"]), res, cont_ids[idx]))
            body = "\n".join(pg["ops"]).encode("latin-1")
            objs[cont_ids[idx]] = ("RAW", b"<< /Length %d >>\nstream\n" % len(body)
                                   + body + b"\nendstream")
        objs[info_id] = ("<< /Title (%s) /Producer (songbook.py) "
                         "/CreationDate (D:%s) >>" % (
                             _esc(title), "20240101000000"))

        out = bytearray(b"%PDF-1.4\n%\xe2\xe3\xcf\xd3\n")
        offsets = {}
        for i in range(1, info_id + 1):
            offsets[i] = len(out)
            obj = objs[i]
            out += b"%d 0 obj\n" % i
            if isinstance(obj, tuple):
                out += obj[1]
            else:
                out += obj.encode("latin-1")
            out += b"\nendobj\n"
        xref = len(out)
        out += b"xref\n0 %d\n0000000000 65535 f \n" % (info_id + 1)
        for i in range(1, info_id + 1):
            out += b"%010d 00000 n \n" % offsets[i]
        out += b"trailer\n<< /Size %d /Root 1 0 R /Info %d 0 R >>\nstartxref\n%d\n%%%%EOF\n" % (
            info_id + 1, info_id, xref)
        with open(path, "wb") as f:
            f.write(bytes(out))

# --------------------------------------------------------------------------
# 3. PNG loader (for optional logos)
# --------------------------------------------------------------------------
def load_png(path, bg=(1, 1, 1)):
    d = open(path, "rb").read()
    if d[:8] != b"\x89PNG\r\n\x1a\n":
        raise ValueError("not a PNG")
    pos = 8
    idat = b""
    w = h = bitd = ctype = None
    plte = None
    while pos < len(d):
        ln = int.from_bytes(d[pos:pos+4], "big")
        typ = d[pos+4:pos+8]
        chunk = d[pos+8:pos+8+ln]
        pos += 12 + ln
        if typ == b"IHDR":
            w, h, bitd, ctype = (int.from_bytes(chunk[0:4], "big"),
                                 int.from_bytes(chunk[4:8], "big"),
                                 chunk[8], chunk[9])
            if chunk[12] != 0:
                raise ValueError("interlaced PNG not supported")
        elif typ == b"PLTE":
            plte = chunk
        elif typ == b"IDAT":
            idat += chunk
        elif typ == b"IEND":
            break
    if bitd != 8:
        raise ValueError("only 8-bit PNG supported")
    raw = zlib.decompress(idat)
    nch = {0: 1, 2: 3, 3: 1, 4: 2, 6: 4}[ctype]
    stride = w * nch
    out = bytearray()
    prev = bytearray(stride)
    p = 0
    for _ in range(h):
        f = raw[p]; p += 1
        line = bytearray(raw[p:p+stride]); p += stride
        if f == 1:
            for i in range(nch, stride):
                line[i] = (line[i] + line[i-nch]) & 255
        elif f == 2:
            for i in range(stride):
                line[i] = (line[i] + prev[i]) & 255
        elif f == 3:
            for i in range(stride):
                a = line[i-nch] if i >= nch else 0
                line[i] = (line[i] + ((a + prev[i]) >> 1)) & 255
        elif f == 4:
            for i in range(stride):
                a = line[i-nch] if i >= nch else 0
                b = prev[i]
                c = prev[i-nch] if i >= nch else 0
                pp = a + b - c
                pa, pb, pc = abs(pp-a), abs(pp-b), abs(pp-c)
                pr = a if (pa <= pb and pa <= pc) else (b if pb <= pc else c)
                line[i] = (line[i] + pr) & 255
        out += line
        prev = line
    rgb = bytearray()
    for i in range(w * h):
        px = out[i*nch:(i+1)*nch]
        if ctype == 0:
            r = g = b = px[0]; a = 255
        elif ctype == 2:
            r, g, b = px[0], px[1], px[2]; a = 255
        elif ctype == 3:
            idx = px[0] * 3
            r, g, b = plte[idx], plte[idx+1], plte[idx+2]; a = 255
        elif ctype == 4:
            r = g = b = px[0]; a = px[1]
        else:
            r, g, b, a = px[0], px[1], px[2], px[3]
        if a != 255:
            r = (r * a + int(bg[0] * 255) * (255 - a)) // 255
            g = (g * a + int(bg[1] * 255) * (255 - a)) // 255
            b = (b * a + int(bg[2] * 255) * (255 - a)) // 255
        rgb += bytes((r, g, b))
    return w, h, bytes(rgb)

def load_logo(path, bg):
    ext = os.path.splitext(path)[1].lower()
    if ext == ".png":
        return load_png(path, bg)
    raise ValueError("only PNG logos are supported")

# --------------------------------------------------------------------------
# 4. Import (the app's songbook format + CSV)
# --------------------------------------------------------------------------
PAPERS = {                      # inches, exactly as the app defines them
    "a4":     (8.3, 11.7),
    "letter": (8.5, 11.0),
    "long":   (8.5, 13.0),
    "legal":  (8.5, 14.0),
}
MARGINS = [0.20, 0.25, 0.30, 0.40, 0.50]
# rows per page and y of the first row - hard-coded table from the app
ROWS = {
    "a4":     {0.20: (39, 58), 0.25: (39, 58), 0.30: (39, 58), 0.40: (38, 68), 0.50: (37, 77)},
    "letter": {0.20: (37, 52), 0.25: (36, 62), 0.30: (36, 62), 0.40: (35, 72), 0.50: (35, 71)},
    "long":   {0.20: (44, 57), 0.25: (44, 58), 0.30: (44, 58), 0.40: (43, 67), 0.50: (42, 77)},
    "legal":  {0.20: (48, 56), 0.25: (48, 56), 0.30: (47, 66), 0.40: (47, 65), 0.50: (46, 75)},
}
ROW_H = 19          # app: this._vvv4 = 19
GAP = 8             # gap between columns
NUM_W = 40          # number column width (42 if numbers > 9 999 999)
DEFAULT_ARTIST = "(unknown)"

class Song:
    __slots__ = ("num", "title", "artist", "genre", "stype", "release", "line")
    def __init__(self, num, title, artist, genre="", stype="", release="", line=0):
        self.num, self.title, self.artist = num, title, artist
        self.genre, self.stype, self.release, self.line = genre, stype, release, line

def read_text(path):
    raw = open(path, "rb").read()
    for enc in ("utf-8-sig", "utf-8", "cp1252", "latin-1"):
        try:
            return raw.decode(enc)
        except UnicodeDecodeError:
            continue
    return raw.decode("latin-1", "replace")

def split_lines(text):
    return [l for l in re.split(r"\r\n|\r|\n", text)]

def parse_songs(text, order="auto", verbose=True):
    lines = [l for l in split_lines(text)]
    sample = [l for l in lines if l.strip()][:50] or [""]
    # delimiter detection: pick the delimiter that gives the most columns
    best, best_cols = None, 0
    for d in ("|", "\t", ";", ","):
        cols = [len(l.split(d)) for l in sample]
        if cols and min(cols) >= 2 and sum(cols) / len(cols) >= 3:
            score = sum(1 for c in cols if c >= 3)
            if score > best_cols:
                best, best_cols = d, score
    if best is None:
        best = "|"

    songs, errors = [], {"invalid": [], "notitle": [], "badnum": [], "dupe": []}
    seen = set()
    if best in ",;":
        table = list(csv.reader([l for l in lines if l.strip()], delimiter=best,
                                skipinitialspace=True))
        sources = [l for l in lines if l.strip()]
    else:
        table = [[c.strip().strip('"') for c in l.split(best)]
                 for l in lines if l.strip()]
        sources = [l for l in lines if l.strip()]
    for i, cols in enumerate(table):
        raw = sources[i] if i < len(sources) else ""
        cols = [c.strip().strip('"') for c in cols]
        if len(cols) < 3:
            errors["invalid"].append((i + 1, raw)); continue
        c0, c1, c2 = cols[0], cols[1], cols[2]
        if not c0 or not c1:
            errors["invalid"].append((i + 1, raw)); continue
        if not c2:
            c2 = DEFAULT_ARTIST
        o = order
        if o == "auto":
            if re.fullmatch(r"-?\d+(\.0+)?", c0):
                o = "nta"
            elif re.fullmatch(r"-?\d+(\.0+)?", c1):
                o = "tna"
            elif re.fullmatch(r"-?\d+(\.0+)?", c2):
                o = "atn"
            else:
                o = "nta"
        if o == "nta":
            ns, ts, as_ = c0, c1, c2
        elif o == "tna":
            ns, ts, as_ = c1, c0, c2
        else:
            ns, ts, as_ = c1, c2, c0
        try:
            num = int(float(ns))
        except ValueError:
            errors["badnum"].append((i + 1, raw)); continue
        if not ts:
            errors["notitle"].append((i + 1, raw)); continue
        if num in seen:
            errors["dupe"].append((i + 1, raw)); continue
        seen.add(num)
        genre = cols[3] if len(cols) > 3 else ""
        stype = cols[4] if len(cols) > 4 else ""
        rel = cols[5] if len(cols) > 5 else ""
        if rel == "user content":
            continue                      # the app keeps these out of the song list
        if "," not in as_:
            as_ = as_.replace(";", ",")    # app: normalise ';' unless ',' present
        songs.append(Song(num, ts, as_, genre, stype, rel, i + 1))

    if verbose:
        for key, label in (("invalid", "Invalid line"), ("notitle", "No title"),
                           ("badnum", "Invalid number"), ("dupe", "Number exists")):
            if errors[key]:
                print("REJECTED: %s (%d)" % (label, len(errors[key])), file=sys.stderr)
                for ln, raw in errors[key][:20]:
                    print("  line %d: %s" % (ln, raw[:100]), file=sys.stderr)
                if len(errors[key]) > 20:
                    print("  ... more.", file=sys.stderr)
    return songs, errors

# --------------------------------------------------------------------------
# 5. Layout / rendering (1:1 with pprint.bas)
# --------------------------------------------------------------------------
def hexcolor(s, default):
    if not s:
        return default
    s = s.strip().lstrip("#")
    if len(s) == 8:
        s = s[2:]
    if len(s) != 6:
        return default
    return tuple(int(s[i:i+2], 16) / 255.0 for i in (0, 2, 4))

def blend(fg, bg, alpha):
    return tuple(fg[i] * alpha + bg[i] * (1 - alpha) for i in range(3))

class Options:
    pass

def sort_songs(songs, key):
    if key == "artist":
        return sorted(songs, key=lambda s: (s.artist.casefold(), s.title.casefold(), s.num))
    if key == "number":
        return sorted(songs, key=lambda s: (s.num, s.title.casefold()))
    return sorted(songs, key=lambda s: (s.title.casefold(), s.artist.casefold(), s.num))

def section_key(s, method):
    v = s.artist if method == "artist" else s.title
    v = v.strip()
    if not v:
        return "#"
    ch = v[0].upper()
    return ch if ch.isalpha() else "#"

def section_order(ch):
    # A-Z first, then '#' for number/symbol titles (app: str2.toUpperCase())
    return (1, ch) if ch == "#" else (0, ch)

def paginate(items, per_page, sections_mode):
    """Split [(kind, payload)] into pages of at most per_page items."""
    pages, cur = [], []
    for it in items:
        cur.append(it)
        if len(cur) == per_page:
            pages.append(cur); cur = []
    if cur or not pages:
        pages.append(cur)
    if sections_mode == "newpage":
        out, cur = [], []
        for it in items:
            if it[0] == "letter" and cur:
                out.append(cur); cur = []
            cur.append(it)
            while len(cur) >= per_page:
                out.append(cur[:per_page]); cur = cur[per_page:]
        if cur:
            out.append(cur)
        pages = out or [[]]
    return pages

def build_pdf(songs, opt):
    W = int(PAPERS[opt.paper][0] * 72)
    H = int(PAPERS[opt.paper][1] * 72)
    m = int(opt.margin * 72)
    rows, top = ROWS[opt.paper][opt.margin]
    bottom = top + rows * ROW_H

    cols = opt.columns
    col_w = (W - 2 * m - (cols - 1) * GAP) / float(cols)
    cell_x = []
    x = m
    for _ in range(cols):
        cell_x.append(int(x))
        x = x + col_w + GAP
    rest = int(col_w - NUM_W)
    title_w = int(0.65 * rest)
    artist_w = int(0.35 * rest)
    num_w = NUM_W
    maxnum = max((s.num for s in songs), default=0)
    if maxnum > 9999999:
        num_w = NUM_W + 2
        title_w -= 2
    widths = [num_w, title_w, artist_w] if opt.order != "tna" else [title_w, num_w, artist_w]
    if opt.no_artist:
        widths = [num_w, title_w + artist_w, 0] if opt.order != "tna" else [title_w + artist_w, num_w, 0]
        if opt.order == "tna":
            widths = [title_w + artist_w, num_w, 0]
    total_w = sum(widths)
    if cols == 4 and not opt.no_artist:
        print("warning: 4 columns leaves only %dpt for the artist cell "
              "(the app only allows 4 columns when the artist column is dropped)"
              % artist_w, file=sys.stderr)

    page_bg = hexcolor(opt.page_color, (1, 1, 1))
    line_c = hexcolor(opt.line_color, (0xBD/255.0, 0xBD/255.0, 0xBD/255.0))
    band_c = hexcolor(opt.band_color, (0, 0, 0))
    band_a = opt.band_opacity / 255.0
    num_c = hexcolor(opt.num_color, (0, 0, 0))
    title_c = hexcolor(opt.title_color, (0, 0, 0))
    artist_c = hexcolor(opt.artist_color, (0, 0, 0))
    sect_bg = hexcolor(opt.section_bg, (0.93, 0.93, 0.93))
    sect_c = hexcolor(opt.section_color, (0, 0, 0))
    header_c = hexcolor(opt.header_color, (0, 0, 0))
    footer_c = hexcolor(opt.footer_color, (0, 0, 0))
    cell_bg = [hexcolor(opt.num_bg, None), hexcolor(opt.title_bg, None), hexcolor(opt.artist_bg, None)]

    scale = max(opt.scale, 0.1)
    sz_num = opt.num_size / scale
    sz_title = opt.title_size / scale
    sz_artist = opt.artist_size / scale
    sz_letter = opt.letter_size / scale
    sz_header = opt.header_size / scale
    sz_footer = opt.footer_size / scale
    min_size = 9.0 / scale
    step = 0.25 / scale

    # ---- build the flow -------------------------------------------------
    ordered = sort_songs(songs, opt.sort)
    if opt.sections == "none":
        items = [("song", s) for s in ordered]
    else:
        items, last = [], None
        if opt.sections == "continuous":
            ordered = sorted(ordered, key=lambda s: section_order(section_key(s, opt.sort)))
        for s in ordered:
            k = section_key(s, opt.sort)
            if k != last:
                items.append(("letter", k))
                last = k
            items.append(("song", s))
    per_page = rows * cols
    pages = paginate(items, per_page, opt.sections)
    total_pages = len(pages)

    pdf = PDF()
    logo = None
    if opt.logo:
        try:
            logo = load_logo(opt.logo, (1, 1, 1))
            pdf.images["Im1"] = logo
        except Exception as e:
            print("warning: could not load logo: %s" % e, file=sys.stderr)

    # ---- draw one row cell ---------------------------------------------
    def fit(s_txt, size, maxw, bold):
        """Shrink a string until it fits, then truncate as a last resort."""
        floor = min(min_size, max(2.6, size * 0.45))
        sz = size
        while twidth(s_txt, sz, bold) > maxw and sz > floor:
            sz -= step
        while s_txt and twidth(s_txt, sz, bold) > maxw:
            s_txt = s_txt[:-1]
        return s_txt, sz

    def draw_cell_text(s_txt, cx, w_cell, y_top, size, color, bold, max_extra=0.0):
        """Shrink-to-fit + optional 2-line break, like pprint._vvv3."""
        y_bot = y_top + ROW_H
        maxw = w_cell - 3 - max_extra
        sz = size
        while twidth(s_txt, sz, bold) > maxw and sz > min_size:
            sz -= step
        two, line2 = False, ""
        if twidth(s_txt, sz, bold) > maxw:
            # split at the last space/hyphen that still fits (pprint._vvv3)
            buf, lastb, i = "", -1, 0
            while i < len(s_txt):
                ch = s_txt[i]
                buf += ch
                if ch in " -":
                    lastb = i
                if twidth(buf, sz, bold) > maxw:
                    if lastb < 0:
                        line2 = s_txt[i + 1:]
                        s_txt = buf
                    else:
                        line2 = s_txt[lastb + 1:]
                        s_txt = s_txt[:lastb + 1]
                    two = True
                    break
                i += 1
        if two and line2.strip():
            l1, s1 = fit(s_txt.strip(), sz, maxw, bold)
            l2, s2 = fit(line2.strip(), sz, maxw, bold)
            pdf.text(l1, cx + 2, y_bot - ROW_H * 0.58, s1, color, bold, "left")
            pdf.text(l2, cx + 2, y_bot - ROW_H * 0.16, s2, color, bold, "left")
        else:
            l1, s1 = fit(s_txt, sz, maxw, bold)
            pdf.text(l1, cx + 2, y_bot - ROW_H / 2 + 0.45 * s1, s1, color, bold, "left")

    def draw_number(num, cx, w_cell, y_top):
        s_txt, sz = fit(str(num), sz_num, w_cell - 1, opt.bold)
        pdf.text(s_txt, cx + w_cell / 2.0,
                 y_top + ROW_H / 2 + 0.45 * sz, sz, num_c, opt.bold, "center")

    def draw_letter(letter, cx, y_top):
        pdf.fill_rect(cx - 1, y_top - 1, total_w - 2, ROW_H + 2, sect_bg)
        pdf.stroke_rect(cx - 1, y_top - 1, total_w - 2, ROW_H + 2, (0, 0, 0), 0.2)
        txt, sz = fit(letter, sz_letter, total_w - 2, True)
        pdf.text(txt, cx - 1 + (total_w - 2) / 2.0,
                 y_top + ROW_H / 2 + 0.45 * sz, sz, sect_c, True, "center")

    def draw_song(s, cx, y_top):
        for ci in range(3):
            w_cell = widths[ci]
            if w_cell <= 0:
                continue
            x = cx + sum(widths[:ci])
            if cell_bg[ci] is not None:
                pdf.fill_rect(x, y_top, w_cell, ROW_H, cell_bg[ci])
            pdf.stroke_rect(x, y_top, w_cell, ROW_H, line_c, 0.2)
        if opt.order == "tna":
            draw_cell_text(s.title, cx + widths[0], widths[1], y_top, sz_title, title_c, opt.bold)
            draw_number(s.num, cx, widths[0], y_top)
        else:
            draw_number(s.num, cx, widths[0], y_top)
            draw_cell_text(s.title, cx + widths[0], widths[1], y_top, sz_title, title_c, opt.bold)
        if not opt.no_artist:
            draw_cell_text(s.artist, cx + widths[0] + widths[1], widths[2],
                           y_top, sz_artist, artist_c, opt.bold)

    def shrink_to_fit(s_txt, size, maxw, bold):
        return fit(s_txt, size, maxw, bold)[1]

    # ---- pages ----------------------------------------------------------
    for pno, page_items in enumerate(pages, start=1):
        pdf.page(W, H)
        pdf.fill_rect(0, 0, W, H, page_bg)
        # column background bands (drawn before the rows)
        for c in range(cols):
            for ci in range(3):
                if widths[ci] <= 0 or cell_bg[ci] is None:
                    continue
                x = cell_x[c] + sum(widths[:ci])
                pdf.fill_rect(x, top, widths[ci], bottom - top, cell_bg[ci])
        # rows
        flow = page_items
        if opt.sections == "newpage":
            # "Start a new page": the letter goes in the page header, not in a row
            flow = [it for it in page_items if it[0] == "song"]
        pos = 0
        for c in range(cols):
            for r in range(rows):
                if pos >= len(flow):
                    break
                kind, payload = flow[pos]; pos += 1
                y_top = top + r * ROW_H
                if kind == "letter":
                    draw_letter(payload, cell_x[c], y_top)
                else:
                    draw_song(payload, cell_x[c], y_top)

        # ---- furniture ------------------------------------------------
        band_top = blend(band_c, page_bg, band_a)
        band_bot = blend(band_c, page_bg, band_a)
        pdf.fill_rect(m, m, W - 2 * m, 35, band_top)
        pdf.fill_rect(m, H - m - 20, W - 2 * m, 20, band_bot)

        mirror = opt.mirror and pno % 2 == 0
        base_top = m + 17.5 + theight(sz_header) / 2 - 1
        base_bot = H - m - 11 + theight(sz_footer) / 2
        maxw = W - 2 * m - 10

        left_txt = opt.header_left or ""
        right_txt = opt.header_right or ""
        center_txt = opt.header_center if opt.header_center is not None else (opt.name or "")

        if mirror:
            left_txt, right_txt = right_txt, left_txt

        page_letter = None
        if opt.sections == "newpage" and page_items and page_items[0][0] == "letter":
            page_letter = page_items[0][1]
        if page_letter:
            # app: L90_3(str, page) - the section letter replaces the top-left text
            sz = shrink_to_fit(page_letter, sz_header * 1.4, maxw / 3, True)
            pdf.text(page_letter, m + 5, base_top, sz, header_c, True, "left")
        elif left_txt:
            sz = shrink_to_fit(left_txt, sz_header, maxw, opt.bold)
            pdf.text(left_txt, m + 5, base_top, sz, header_c, opt.bold, "left")
        if right_txt:
            sz = shrink_to_fit(right_txt, sz_header, maxw, opt.bold)
            pdf.text(right_txt, W - m - 5, base_top, sz, header_c, opt.bold, "right")
        if center_txt:
            sz = shrink_to_fit(center_txt, sz_header, maxw, opt.bold)
            pdf.text(center_txt, W / 2.0, base_top, sz, header_c, opt.bold, "center")
        if opt.footer_center:
            sz = shrink_to_fit(opt.footer_center, sz_footer, maxw, opt.bold)
            pdf.text(opt.footer_center, W / 2.0, base_bot, sz, footer_c, opt.bold, "center")
        if opt.page_numbers:
            s = "%d / %d" % (pno, total_pages)
            sz = sz_footer
            if mirror:
                pdf.text(s, m + 5, base_bot, sz, footer_c, opt.bold, "left")
            else:
                pdf.text(s, W - m - 5, base_bot, sz, footer_c, opt.bold, "right")
        if logo:
            lw, lh, _ = logo
            box_w = 119.0
            box_h = 20.0
            ar = (lw * box_h) / float(lh)
            if mirror:
                pdf.image("Im1", W - m - 5 - box_w, H - m - 20, box_w, box_h)
            else:
                pdf.image("Im1", m + 5, H - m - 20, box_w, box_h)

    return pdf, total_pages, (W, H, m, rows, top, cols, widths)

# --------------------------------------------------------------------------
# 6. CLI
# --------------------------------------------------------------------------
def main(argv=None):
    ap = argparse.ArgumentParser(
        description="Convert a karaoke song list (CSV/TXT) into a printable "
                    "songbook PDF.",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog=__doc__)
    ap.add_argument("input", help="CSV/TXT file: number|title|artist[|category|type|update]")
    ap.add_argument("-o", "--output", default=None, help="output PDF (default: <input>.pdf)")
    ap.add_argument("--paper", default="a4", choices=sorted(PAPERS), help="paper size (default a4)")
    ap.add_argument("--margin", default=0.25, type=float, choices=MARGINS, help="margin in inches")
    ap.add_argument("--columns", default=3, type=int, choices=(2, 3, 4), help="columns per page")
    ap.add_argument("--order", default="auto", choices=("auto", "nta", "tna", "atn"),
                    help="column order: number-title-artist / title-number-artist / "
                         "artist-title-number (default auto)")
    ap.add_argument("--sort", default="title", choices=("title", "artist", "number"),
                    help="sort key / what the section letters follow (default title)")
    ap.add_argument("--sections", default="continuous", choices=("continuous", "newpage", "none"),
                    help="section letters: continuous | start a new page | none")
    ap.add_argument("--no-artist", action="store_true", help="omit the artist column")
    ap.add_argument("--name", default=None, help="text centred in the top band")
    ap.add_argument("--header-left", default=None)
    ap.add_argument("--header-right", default=None)
    ap.add_argument("--header-center", default=None)
    ap.add_argument("--footer-center", default=None)
    ap.add_argument("--no-page-numbers", dest="page_numbers", action="store_false")
    ap.add_argument("--mirror", action="store_true",
                    help="alternate header/footer sides on even pages (booklet printing)")
    ap.add_argument("--logo", default=None, help="PNG drawn in the bottom band")
    ap.add_argument("--bold", action="store_true", help="bold number/title/artist")
    ap.add_argument("--scale", default=1.0, type=float,
                    help="divide every font size by this (app: device scale, e.g. 2.75)")
    ap.add_argument("--num-size", default=7.5, type=float)
    ap.add_argument("--title-size", default=8.0, type=float)
    ap.add_argument("--artist-size", default=6.5, type=float)
    ap.add_argument("--letter-size", default=11.0, type=float)
    ap.add_argument("--header-size", default=9.0, type=float)
    ap.add_argument("--footer-size", default=6.5, type=float)
    ap.add_argument("--page-color", default=None, help="hex RRGGBB or AARRGGBB")
    ap.add_argument("--line-color", default=None, help="cell border colour (default BDBDBD)")
    ap.add_argument("--band-color", default=None, help="header/footer band colour (default 000000)")
    ap.add_argument("--band-opacity", default=34, type=int, help="band alpha 0-255 (default 34)")
    ap.add_argument("--num-color", default=None)
    ap.add_argument("--title-color", default=None)
    ap.add_argument("--artist-color", default=None)
    ap.add_argument("--section-bg", default=None)
    ap.add_argument("--section-color", default=None)
    ap.add_argument("--header-color", default=None)
    ap.add_argument("--footer-color", default=None)
    ap.add_argument("--num-bg", default=None)
    ap.add_argument("--title-bg", default=None)
    ap.add_argument("--artist-bg", default=None)
    ap.add_argument("--quiet", action="store_true")
    ap.set_defaults(page_numbers=True)
    opt = ap.parse_args(argv)

    text = read_text(opt.input)
    songs, errors = parse_songs(text, opt.order, verbose=not opt.quiet)
    if not songs:
        print("error: no valid songs found", file=sys.stderr)
        return 2

    pdf, total_pages, geo = build_pdf(songs, opt)
    out = opt.output or os.path.splitext(opt.input)[0] + ".pdf"
    pdf.save(out, title=opt.name or os.path.basename(out))
    if not opt.quiet:
        W, H, m, rows, top, cols, widths = geo
        print("songs: %d   pages: %d   page: %dx%dpt (%s)   columns: %d   "
              "rows/col: %d   cell widths: %s" % (
                  len(songs), total_pages, W, H, opt.paper, cols, rows,
                  "/".join(str(w) for w in widths)))
        print("wrote %s" % out)
    return 0

if __name__ == "__main__":
    sys.exit(main())
