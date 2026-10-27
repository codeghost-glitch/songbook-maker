# MEGA Karaoke Songbook 2.59 — the songbook engine, reversed

Target: `MEGA-Karaoke-Songbook_2.59_apkcube.apks` → package **app.lcs.megabook**,
versionName 2.59, versionCode 259, minSdk 23, targetSdk 36.

The app is written in **B4A (Basic4Android)** and compiled to Java with every identifier
obfuscated (`pprint._vvvvvvvv…4()` …), so the analysis was done on jadx output of the
142 `app.lcs` classes. Two classes matter:

| class | role |
|---|---|
| `pprint` (8.8k lines) | the PDF songbook renderer (`PdfDocumentWrapper` + `CanvasWrapper`) |
| `sbmanager` (7.5k lines) | songbook import / export / rename / merge |

Below is everything needed to reproduce "CSV → PDF" without the app. A working
re-implementation is in **`songbook.py`** (single file, no dependencies).

---

## 1. Import format

Help text shown by the app (`sbmanager`, `_btnimportinfo_click`):

```
Mega Karaoke Songbook can import data from a text file (or compressed text)
with 3 or 6 columns in the following format:

number | title | artist
Or
number | title | artist | category | song type | update

Column delimiter can be:
Pipe ( | )
Tab
```

### Delimiter detection
`sbmanager._v34_2()` inspects **only the first line**:

* contains a TAB → delimiter = TAB
* else contains `|` → delimiter = `\|`
* else → error *"Please select a file which columns are delimited by any of the following: 1. pipe ( | ) 2. TAB"*

The file is copied to `temp.txt`, every `"` character is deleted
(`text.replace(Common.QUOTE, "")`), then read with `File.ReadList` and split per line
with `Regex.Split(delimiter, line)`. Requires at least 3 columns
(*"File must have at least 3 columns."*).

### Column order (`_format`)
| value | order |
|---|---|
| 0 | number, title, artist |
| 1 | title, number, artist |
| 2 | artist, title, number |

### Validation, exactly as implemented
| condition | result / message |
|---|---|
| < 3 columns | `Invalid line` |
| number or title empty | `Invalid line` |
| artist empty | artist := `"(unknown)"` (`main._v24_1`) |
| number not parseable | `Invalid number` (parsed as `double` then cast to int, so `1001.0` is valid) |
| duplicate number in the file | `Number exists` (SQLite `tempor10 (number INTEGER UNIQUE, …)`) |
| `update` column == `user content` | row is **skipped** (goes to the "user content" songbook) |
| artist contains `;` but no `,` | `;` → `,` |
| artist list (`A;B`) | additionally split into one DB row per artist (same number), kept in column `solo` |

Rejected lines are reported in a dialog, max 100 per category:
`REJECTED:`, then `Invalid line (n)`, `No title (n)`, `Invalid number (n)`,
`Number exists (n)`, followed by `Line N:  <raw line>`.

### Storage
`players` table — one row per songbook:
`name, tablename, delimiter, columns, <tablename>, author, players, notes, …, rev, date`

Each songbook is its own table:

```sql
CREATE TABLE <tablename> (
  number INTEGER, title TEXT, artist TEXT, category TEXT,
  type TEXT, release TEXT, solo TEXT)
```

The per-songbook delimiter is remembered — export reuses it.

## 2. Export formats (`_btnexportok_click`)

* `.txt` — same columns as the table, delimiter = the songbook's stored delimiter,
  **but `;` is written as TAB**; UTF-8.
* `juke.txt` — prompt *"Export 3 columns only? number | title | artist"* →
  **pipe**-delimited, 3 columns (CAVS karaoke players).
* `.xls` — jExcelApi workbook, data sheet + `info` sheet.
* player `.xml` — `<songbook><name>…</name><author>…</author><players>…</players><notes>…</notes></songbook>`.

## 3. PDF engine (`pprint`)

### Paper / margins
| paper | inches | points |
|---|---|---|
| A4 | 8.3 × 11.7 | 597 × 842 |
| Letter | 8.5 × 11 | 612 × 792 |
| Long | 8.5 × 13 | 612 × 936 |
| Legal | 8.5 × 14 | 612 × 1008 |

Margins: `0.20 / 0.25 / 0.30 / 0.40 / 0.50 in` → `×72` pt.
A4 is deliberately 8.3 in, not 8.27.

### Rows per page and y of the first row (hard-coded table, `_v90_4()`)

| paper | 0.20 | 0.25 | 0.30 | 0.40 | 0.50 |
|---|---|---|---|---|---|
| A4 | 39, 58 | 39, 58 | 39, 58 | 38, 68 | 37, 77 |
| Letter | 37, 52 | 36, 62 | 36, 62 | 35, 72 | 35, 71 |
| Long | 44, 57 | 44, 58 | 44, 58 | 43, 67 | 42, 77 |
| Legal | 48, 56 | 48, 56 | 47, 66 | 47, 65 | 46, 75 |

### Grid
```
rowHeight   = 19 pt                     (fixed)
columnGap   = 8 pt
colWidth    = (pageW - 2*margin - (cols-1)*8) / cols
columnX[i]  = margin + i*(colWidth + 8)          // integer accumulation, as in the app
rest        = colWidth - 40
numberCol   = 40 pt   (42 if max(number) > 9 999 999, titleCol -= 2)
titleCol    = 0.65 * rest
artistCol   = 0.35 * rest
order "Number|Title|Artist" -> [num, title, artist]
order "Title|Number|Artist" -> [title, num, artist]
```
Column count: 2 or 3, and **4 only for artist songbooks** (which drop the artist column).
Rows fill column 1 top→bottom, then column 2, …, then the next page.

### Fonts / shrink-to-fit
Sizes are chosen in `sp` and divided by the device scale
(`GetDeviceLayoutValues.Scale`): `pt = size / scale`.

| element | start | min | step |
|---|---|---|---|
| number | 20/sp | 9/sp | 0.25/sp |
| title | 20/sp | 9/sp | 0.25/sp |
| artist | 17/sp | 9/sp | 0.25/sp |
| section letter | 16/sp | 9/sp | 0.25/sp |
| header/footer | 14–26/sp | — | shrink to page width − 10 |

Fit rule (`_v87_3`): measure width; while `width > cellWidth - 3` subtract 0.25/sp;
if the minimum is reached, split at the **last space or hyphen that still fits** and draw
two lines (baselines `rowBottom - 11` and `rowBottom - 3`).
Numbers are centred in the 40 pt box.

### Cell / page decoration
* cell border: 0.2 pt stroke, `lineColor` (default `0xFFBDBDBD`)
* page colour: `pageColor` (default white)
* section letter box: full column width, 0.2 pt black outline, `sectionBg` fill
* top band: 35 pt tall, colour `topBg` at `opacityTop/255` (default black @ 34/255 ≈ 13 %)
* bottom band: 20 pt tall, `bottomBg` @ `opacityBottom`
* logo: `assets/sbprintlogo.png` drawn bottom-left, 119 × 20 pt
* header texts: top-left `x=m+5`, top-centre, top-right (`x=W-m-5`),
  baseline `m + 17.5 + textHeight/2`
* footer: bottom-centre, page number `n / total` bottom-right,
  baseline `pageH - m - 11 + textHeight/2`
* `chkMirror`: on even pages left/right are swapped (booklet printing)

### Section letters (`spnSections`)
| mode | behaviour |
|---|---|
| `Continuous` | letter takes a row **inside** the flow (counts toward the page capacity) |
| `Start a new page` | every new letter starts a new page; the letter is drawn instead of the top-left header text |
| `None` | no letters at all |

The letter is `firstChar.toUpperCase()` of the sort key (title, or artist for artist
songbooks); rows are sorted `COLLATE NOCASE`.

### Other settings (persisted keys)
`chkNumber, chkTitle, chkArtist, chkBold, chkHeader, chkTopLeft, chkTopCenter, chkTopRight,`
`chkImgTopLeft/Center/Right, chkInclude, chkIncludeUserContent, chkShowIcon, chkMirror,`
`spnPaper, spnMargins, spnColumns, spnOrder, spnSections, textSizePrintNumber/Title/Artist,`
`pageColor, lineColor, cSectionColor, cNumberColor, cTitleColor, cArtistColor,`
`cSectionBackground, cNumberBackground, cTitleBackground, cArtistBackground,`
`cTopLeft/TopCenter/TopRight/BottomCenter/BottomRightColor, cTopBgColor, cBottomBgColor,`
`opacityTop, opacityBottom`

---

## 4. `songbook.py` — CSV/TXT → PDF

```bash
python3 songbook.py songs.csv -o songbook.pdf
python3 songbook.py songs.txt --paper letter --margin 0.3 --columns 2 \
        --order tna --sections newpage --name "Rey's Karaoke" -o out.pdf
```

Options (defaults mirror the app):

| option | meaning |
|---|---|
| `--paper a4\|letter\|long\|legal` | page size |
| `--margin 0.2\|0.25\|0.3\|0.4\|0.5` | margin in inches |
| `--columns 2\|3\|4` | columns per page |
| `--order auto\|nta\|tna\|atn` | column order (auto-detected from the numbers) |
| `--sort title\|artist\|number` | sort key, also drives the section letters |
| `--sections continuous\|newpage\|none` | section letter behaviour |
| `--no-artist` | omit the artist column (artist songbook look) |
| `--name`, `--header-left/right/center`, `--footer-center` | printed texts |
| `--no-page-numbers`, `--mirror` | footer page numbers, booklet mirroring |
| `--logo logo.png` | PNG drawn in the bottom band (119 × 20 pt) |
| `--bold` | bold number/title/artist |
| `--num-size/--title-size/--artist-size/--letter-size/--header-size/--footer-size` | point sizes |
| `--scale` | divide every size by this (app device scale, e.g. 2.75) |
| `--page-color/--line-color/--band-color/--band-opacity/--num-color/…` | colours (`RRGGBB` or `AARRGGBB`) |

What it does **not** copy from the app: ads, the in-app database, XLS import/export,
per-songbook "user content" lists, the karaoke-type icons, and the "inner border" path that
the app builds but never strokes.

Deliberate improvements: `,` and `;` are accepted as delimiters (with CSV quoting),
4 columns work for any songbook, and text that still does not fit is truncated instead of
being drawn over the neighbouring cell.

Samples: `samples/songs.txt` (88 rows), `samples/songs400.txt` (400 rows),
`samples/book400.pdf` (3 columns, continuous sections),
`samples/book400np.pdf` (new page per section),
`samples/book4col.pdf` (artist style, 4 columns),
`samples/bookmirror.pdf` (mirror + logo).
