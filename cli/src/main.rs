//! songbook - turn a CSV/TXT karaoke song list into a printable PDF songbook.
//!
//! The layout reproduces MEGA Karaoke Songbook 2.59 for Android
//! (app.lcs.megabook), whose print engine was reverse engineered from the
//! shipped APK. See README.md for the full geometry table.

use std::path::PathBuf;
use std::process::ExitCode;

use songbook::import::{self, Order};
use songbook::pdf::Color;
use songbook::render::{self, Options, Paper, Sections, SortKey};

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn usage() -> String {
    format!(
        "songbook {ver} - songbook PDF maker for karaoke song lists

USAGE:
    songbook <INPUT> [OPTIONS]

ARGS:
    <INPUT>    CSV/TXT file: number|title|artist[|category|type|update]

OPTIONS:
    -o, --output <FILE>        output PDF (default: input name with .pdf)
        --paper <SIZE>         a4 | letter | long | legal          [default: a4]
        --margin <INCH>        0.20 | 0.25 | 0.30 | 0.40 | 0.50    [default: 0.25]
        --columns <N>          2 | 3 | 4                          [default: 3]
        --order <ORDER>        auto | nta | tna | atn              [default: auto]
        --sort <KEY>           title | artist | number             [default: title]
        --sections <MODE>      continuous | newpage | none         [default: continuous]
        --no-artist            drop the artist column (artist songbook look)
        --name <TEXT>          text in the middle of the top band
        --header-left <TEXT>   text in the top left corner
        --header-right <TEXT>  text in the top right corner
        --header-center <TEXT> overrides --name
        --footer-center <TEXT> text in the middle of the bottom band
        --no-page-numbers      hide the 'n / total' counter
        --mirror               swap header/footer sides on even pages
        --logo <PNG>           logo drawn in the bottom band (119x20 pt)
        --bold                 bold number/title/artist
        --scale <F>            divide all font sizes by this (app device scale)
        --num-size <PT>        [default: 7.5]
        --title-size <PT>      [default: 8]
        --artist-size <PT>     [default: 6.5]
        --letter-size <PT>     [default: 11]
        --header-size <PT>     [default: 9]
        --footer-size <PT>     [default: 6.5]
        --page-color <HEX>     RRGGBB or AARRGGBB
        --line-color <HEX>     cell borders                  [default: BDBDBD]
        --band-color <HEX>     header/footer band            [default: 000000]
        --band-opacity <0-255> band alpha                    [default: 34]
        --num-color <HEX>      --title-color <HEX>  --artist-color <HEX>
        --section-bg <HEX>     --section-color <HEX>
        --header-color <HEX>   --footer-color <HEX>
        --num-bg <HEX>         --title-bg <HEX>  --artist-bg <HEX>
        --quiet                only print errors
    -h, --help                 show this help
    -V, --version              show the version

EXAMPLES:
    songbook songs.csv -o songbook.pdf
    songbook songs.txt --paper letter --columns 2 --name 'Karaoke Night'
",
        ver = VERSION
    )
}

struct Args {
    input: PathBuf,
    output: Option<PathBuf>,
    quiet: bool,
    opt: Options,
}

fn next_value<'a>(it: &mut std::slice::Iter<'a, String>, flag: &str) -> Result<&'a String, String> {
    it.next().ok_or_else(|| format!("{} needs a value", flag))
}

fn parse_color(it: &mut std::slice::Iter<'_, String>, flag: &str) -> Result<Color, String> {
    let v = next_value(it, flag)?;
    Color::from_hex(v).ok_or_else(|| format!("{}: invalid colour '{}'", flag, v))
}

fn parse_f32(it: &mut std::slice::Iter<'_, String>, flag: &str) -> Result<f32, String> {
    let v = next_value(it, flag)?;
    v.parse::<f32>().map_err(|_| format!("{}: invalid number '{}'", flag, v))
}

fn parse_args() -> Result<Option<Args>, String> {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    if argv.is_empty() {
        print!("{}", usage());
        return Ok(None);
    }
    let mut opt = Options::default();
    let mut input: Option<PathBuf> = None;
    let mut output: Option<PathBuf> = None;
    let mut quiet = false;

    let mut it = argv.iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "-h" | "--help" => {
                print!("{}", usage());
                return Ok(None);
            }
            "-V" | "--version" => {
                println!("songbook {}", VERSION);
                return Ok(None);
            }
            "-o" | "--output" => output = Some(PathBuf::from(next_value(&mut it, arg)?)),
            "--paper" => {
                let v = next_value(&mut it, arg)?;
                opt.paper = Paper::parse(v).ok_or_else(|| format!("--paper: unknown size '{}'", v))?;
            }
            "--margin" => {
                opt.margin = parse_f32(&mut it, arg)?;
            }
            "--columns" => {
                let v = next_value(&mut it, arg)?;
                opt.columns = v.parse::<usize>().map_err(|_| format!("--columns: invalid '{}'", v))?;
            }
            "--order" => {
                let v = next_value(&mut it, arg)?;
                opt.order = Order::parse(v).ok_or_else(|| format!("--order: unknown value '{}'", v))?;
            }
            "--sort" => {
                let v = next_value(&mut it, arg)?;
                opt.sort = SortKey::parse(v).ok_or_else(|| format!("--sort: unknown key '{}'", v))?;
            }
            "--sections" => {
                let v = next_value(&mut it, arg)?;
                opt.sections = Sections::parse(v).ok_or_else(|| format!("--sections: unknown mode '{}'", v))?;
            }
            "--no-artist" => opt.no_artist = true,
            "--name" => opt.name = next_value(&mut it, arg)?.clone(),
            "--header-left" => opt.header_left = next_value(&mut it, arg)?.clone(),
            "--header-center" => opt.header_center = next_value(&mut it, arg)?.clone(),
            "--header-right" => opt.header_right = next_value(&mut it, arg)?.clone(),
            "--footer-center" => opt.footer_center = next_value(&mut it, arg)?.clone(),
            "--no-page-numbers" => opt.page_numbers = false,
            "--mirror" => opt.mirror = true,
            "--logo" => {
                let path = next_value(&mut it, arg)?.clone();
                let data = std::fs::read(&path).map_err(|e| format!("{}: {}", path, e))?;
                opt.logo = Some(data);
            }
            "--bold" => opt.bold = true,
            "--scale" => opt.scale = parse_f32(&mut it, arg)?,
            "--num-size" => opt.num_size = parse_f32(&mut it, arg)?,
            "--title-size" => opt.title_size = parse_f32(&mut it, arg)?,
            "--artist-size" => opt.artist_size = parse_f32(&mut it, arg)?,
            "--letter-size" => opt.letter_size = parse_f32(&mut it, arg)?,
            "--header-size" => opt.header_size = parse_f32(&mut it, arg)?,
            "--footer-size" => opt.footer_size = parse_f32(&mut it, arg)?,
            "--page-color" => opt.page_color = parse_color(&mut it, arg)?,
            "--line-color" => opt.line_color = parse_color(&mut it, arg)?,
            "--band-color" => opt.band_color = parse_color(&mut it, arg)?,
            "--band-opacity" => {
                let v = next_value(&mut it, arg)?;
                opt.band_opacity = v.parse::<u8>().map_err(|_| format!("--band-opacity: invalid '{}'", v))?;
            }
            "--num-color" => opt.num_color = parse_color(&mut it, arg)?,
            "--title-color" => opt.title_color = parse_color(&mut it, arg)?,
            "--artist-color" => opt.artist_color = parse_color(&mut it, arg)?,
            "--section-bg" => opt.section_bg = parse_color(&mut it, arg)?,
            "--section-color" => opt.section_color = parse_color(&mut it, arg)?,
            "--header-color" => opt.header_color = parse_color(&mut it, arg)?,
            "--footer-color" => opt.footer_color = parse_color(&mut it, arg)?,
            "--num-bg" => opt.num_bg = Some(parse_color(&mut it, arg)?),
            "--title-bg" => opt.title_bg = Some(parse_color(&mut it, arg)?),
            "--artist-bg" => opt.artist_bg = Some(parse_color(&mut it, arg)?),
            "--quiet" => quiet = true,
            other if other.starts_with('-') => return Err(format!("unknown option '{}'", other)),
            other => {
                if input.is_some() {
                    return Err(format!("unexpected extra argument '{}'", other));
                }
                input = Some(PathBuf::from(other));
            }
        }
    }

    let input = input.ok_or("missing input file (try --help)")?;
    Ok(Some(Args { input, output, quiet, opt }))
}

fn run() -> Result<(), String> {
    let args = match parse_args()? {
        Some(a) => a,
        None => return Ok(()),
    };

    let text = std::fs::read_to_string(&args.input)
        .or_else(|_| std::fs::read(&args.input).map(|b| String::from_utf8_lossy(&b).to_string()))
        .map_err(|e| format!("{}: {}", args.input.display(), e))?;

    let (songs, report) = import::parse(&text, args.opt.order);
    if !args.quiet && report.rejected() > 0 {
        eprint!("REJECTED:
{}", report.render(20));
    }
    if songs.is_empty() {
        return Err("no valid songs found".to_string());
    }

    let (pdf, summary) = render::render(&songs, &args.opt)?;

    let out = args
        .output
        .clone()
        .unwrap_or_else(|| args.input.with_extension("pdf"));
    pdf.save(&out, if args.opt.name.is_empty() { "Songbook" } else { &args.opt.name })
        .map_err(|e| format!("{}: {}", out.display(), e))?;

    if !args.quiet {
        println!(
            "songs: {}   pages: {}   page: {}x{}pt ({})   columns: {}   rows/col: {}{}",
            summary.songs,
            summary.pages,
            summary.page_w,
            summary.page_h,
            summary.paper,
            summary.columns,
            summary.rows,
            if report.user_content > 0 {
                format!("   user content skipped: {}", report.user_content)
            } else {
                String::new()
            }
        );
        println!(
            "cell widths: {}/{}/{}   sort: {}   order: {}   sections: {}",
            summary.cells[0],
            summary.cells[1],
            summary.cells[2],
            args.opt.sort.name(),
            args.opt.order.name(),
            args.opt.sections.name()
        );
        println!("wrote {}", out.display());
    }
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {}", e);
            ExitCode::FAILURE
        }
    }
}
