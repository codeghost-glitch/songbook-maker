<div align="center">

# Songbook Maker

**Turn a karaoke song list into a print-ready PDF songbook.**

A Rust engine with a command-line front end and a Kotlin Android app, reproducing the
print engine of [MEGA Karaoke Songbook 2.59](https://play.google.com/store/apps/details?id=app.lcs.megabook).

</div>

<p align="center">
  <img src="docs/android-light.png" width="280" alt="Android app, light theme">
  <img src="docs/android-dark.png" width="280" alt="Android app, dark theme">
</p>

---

## What it does

Feed it a plain text or CSV song list:

```csv
10001,Bohemian Rhapsody,Queen
10002,Imagine,John Lennon
10003,Dancing Queen,ABBA
```

and get a paginated, alphabetic songbook PDF with a header band, letter sections,
three columns of numbered cells and a page counter - the same layout the Android app
prints, minus the ads and the built-in database.

| | |
|---|---|
| Paper | A4 8.3 x 11.7 in, Letter 8.5 x 11 in, Long 8.5 x 13 in, Legal 8.5 x 14 in |
| Margins | 0.20 / 0.25 / 0.30 / 0.40 / 0.50 in (hard-coded row counts per combination) |
| Grid | 2-4 columns, 19 pt rows, 8 pt column gap |
| Cells | number 40 pt (42 pt above 9 999 999), title 65% and artist 35% of the rest |
| Bands | 35 pt header, 20 pt footer, black at 34/255 opacity, optional 119x20 pt logo |
| Text | Helvetica 7.5 / 8 / 6.5 pt, shrink-to-fit with a two-line fallback |

## Architecture

One engine, two front ends:

```
                 +---------------------+
   CSV/TXT ----> |  cli/src/lib.rs     | ----> PDF bytes
                 |  import -> render   |
                 |  pdf writer         |
                 +----------+----------+
                            | JNI (android feature)
                 +----------v----------+
                 |  Kotlin app         |
                 |  Engine.kt (native) |
                 |  MainActivity.kt UI |
                 +---------------------+
```

The CLI parses flags into `Options`; the app sends the same options as `key=value`
lines. Both end up in `render_text()`, so a songbook is identical no matter which
front end produced it. The Rust side owns parsing, pagination, fonts, PDF writing and
even PNG decoding; Kotlin only does UI, file access and saving.

## Install

### CLI (Rust 1.70+, no dependencies)

```bash
cargo install --path cli          # or: cd cli && cargo build --release
songbook songs.csv -o songbook.pdf --name "Karaoke Night"
```

The PDF is written to the path given with `-o` (input name with `.pdf` by default).

### Android (API 23+)

Needs the Android SDK/NDK, `cargo-ndk` and the Android Rust targets:

```bash
rustup target add aarch64-linux-android armv7-linux-androideabi x86_64-linux-android
cargo install cargo-ndk

cd android
./build-native.sh                 # Rust engine -> app/src/main/jniLibs
./gradlew assembleDebug           # -> app/build/outputs/apk/debug/app-debug.apk
adb install -r app/build/outputs/apk/debug/app-debug.apk
```

**Where the PDF goes:** on Android 10 and newer it is saved to
`Downloads/Songbook/<title>.pdf` through MediaStore, so any file manager and any PDF
viewer can open it. The in-app *Open* button launches a viewer and *Share* sends the
same file. On older releases the file falls back to the app cache and is served
through a content provider.

## Song list format

Any of these delimiters is detected automatically: `|`, TAB, `;`, `,`.
Quoted CSV fields are honoured for `,` and `;`.

| Columns | Layout |
|---|---|
| 3 | `number, title, artist` (auto-detected: also `title, number, artist` and `artist, title, number`) |
| 6 | `number, title, artist, genre, type, release` - the app's own export format |

Rows are validated like the original importer:

| Problem | Reported as |
|---|---|
| fewer than 3 fields, empty number or title | `Invalid line` |
| title missing | `No title` |
| number not numeric | `Invalid number` |
| number already used | `Number exists` |
| sixth column is `user content` | silently skipped |
| artist empty | `(unknown)` |
| artist has `;` but no `,` | `;` becomes `,` |

## CLI options

```
songbook <INPUT> [OPTIONS]

-o, --output <FILE>        output PDF (default: input name with .pdf)
    --paper <SIZE>         a4 | letter | long | legal          [default: a4]
    --margin <INCH>        0.20 | 0.25 | 0.30 | 0.40 | 0.50    [default: 0.25]
    --columns <N>          2 | 3 | 4                          [default: 3]
    --order <ORDER>        auto | nta | tna | atn              [default: auto]
    --sort <KEY>           title | artist | number             [default: title]
    --sections <MODE>      continuous | newpage | none         [default: continuous]
    --no-artist            drop the artist column
    --name <TEXT>          text in the middle of the top band
    --header-left/-right   text in the top corners
    --header-center        overrides --name
    --footer-center        text in the middle of the bottom band
    --no-page-numbers      hide the 'n / total' counter
    --mirror               swap header/footer sides on even pages
    --logo <PNG>           logo drawn in the bottom band
    --bold                 bold song text
    --scale <F>            divide all font sizes by this
    --num-size/--title-size/--artist-size/--letter-size/--header-size/--footer-size <PT>
    --page-color/--line-color/--band-color/--band-opacity/--num-color/... <HEX>
    --quiet, -h, -V
```

Examples:

```bash
songbook examples/songs.txt -o book.pdf --name "Karaoke Night" --header-left "Vol. 1"
songbook examples/songs.txt --paper letter --columns 2 --sections newpage --mirror
songbook examples/songs400.txt --logo examples/logo.png --footer-center "MEGA Songbook"
```

The Android app sends the same options as a `key=value` block, for example:

```
paper=a4
margin=0.25
columns=3
sections=continuous
sort=title
artist=1
bold=1
page_numbers=1
name=Karaoke Night
footer_center=MEGA Songbook
```

## Tests

```bash
cd cli && cargo test        # 29 tests: parser, geometry, PDF primitives, inflate, packet
```

The Android app carries no layout logic of its own - it calls the same
`render_text()` through JNI - so the Rust tests cover both front ends. During the
port, the Kotlin/Java layout results were also compared page by page against the CLI
output at 150 dpi and were pixel identical.

## Repository layout

```
cli/                  Rust engine + CLI
  src/lib.rs          options, render entry point, JNI packet format
  src/main.rs         command line front end
  src/import.rs       song list parser (port of sbmanager.bas)
  src/render.rs       print engine (port of pprint.bas)
  src/pdf.rs          PDF 1.4 writer, inflate, PNG decoding
  src/fonts.rs        Helvetica AFM metrics
  src/android.rs      JNI entry points (feature "android")
android/              Kotlin app, plain views, no AndroidX
  app/src/main/java/io/github/codeghost/songbook/
    MainActivity.kt   UI, file picker, options, saving to Downloads
    Engine.kt         JNI wrapper and packet decoding
    PdfProvider.kt    content provider for opening and sharing
  build-native.sh     cargo ndk -> app/src/main/jniLibs
examples/             sample song lists, logo and the Python reference build
docs/                 reverse-engineering notes and screenshots
```

## Background

The original app is written in B4A and shipped fully obfuscated. The print engine was
recovered by decompiling the APK, normalising the obfuscated fields to readable names
and decoding the layout tables - documented in
[docs/reverse-engineering.md](docs/reverse-engineering.md). Ads, the in-app song
database, XLS import, user-content lists and the karaoke icons are deliberately left
out: this is only the CSV to PDF path.

## License

MIT - see [LICENSE](LICENSE). Not affiliated with the original app.
