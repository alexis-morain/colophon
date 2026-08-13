//! Command line shell over `colophon-core`.

use anyhow::Result;
use clap::Parser;
use colophon_core::{build_album, format, pdf, Album, BuildOptions};
use std::path::PathBuf;

/// Colophon: from a folder of photos to a print-ready album.
#[derive(Parser)]
#[command(name = "colophon", version, about)]
#[command(after_help = FORMAT_HELP.as_str())]
struct Cli {
    /// Folder of photos to build the album from
    #[arg(required_unless_present_any = ["formats", "dump_geometry", "print"])]
    photos: Option<PathBuf>,

    /// Output directory (album.json, album.pdf, thumbnail cache)
    #[arg(short, long, default_value = "album-out")]
    out: PathBuf,

    /// Album title (defaults to the folder name)
    #[arg(short, long)]
    title: Option<String>,

    /// Target number of spreads (double pages) for the finished album
    #[arg(short, long, default_value_t = 48)]
    spreads: usize,

    /// Page format: a preset name, or LARGEURxHAUTEUR in millimetres
    #[arg(short, long, default_value = "carre-21")]
    format: String,

    /// List the available page formats and exit
    #[arg(long)]
    formats: bool,

    /// Print every template's slot geometry as JSON and exit. Feeds the
    /// parity check against the editor's TypeScript port.
    #[arg(long)]
    dump_geometry: bool,

    /// Render album-print.pdf at full resolution (300 dpi) from the album
    /// already built in --out. Reopens the originals: slower than the
    /// preview, and the photo folder must still be in place.
    #[arg(long)]
    print: bool,
}

/// Built once at startup so `--help` can show the format table.
static FORMAT_HELP: std::sync::LazyLock<String> = std::sync::LazyLock::new(|| {
    format!("Formats de page :\n{}\n\nOu une taille libre, par exemple --format 240x180.", format::help())
});

fn main() -> Result<()> {
    let cli = Cli::parse();
    if cli.formats {
        println!("{}", format::help());
        return Ok(());
    }

    let trim = format::parse(&cli.format)?;

    if cli.dump_geometry {
        let album = Album::new("geometry", std::path::Path::new("."), trim);
        println!("{}", serde_json::to_string_pretty(&pdf::dump_geometry(&album))?);
        return Ok(());
    }

    if cli.print {
        let t0 = std::time::Instant::now();
        let out = colophon_core::render_print_pdf(
            &cli.out,
            &cli.out.join("album-print.pdf"),
            &|line| eprintln!("{line}"),
        )?;
        eprintln!("done in {:.1?}: {}", t0.elapsed(), out.display());
        return Ok(());
    }

    let photos = cli.photos.clone().expect("clap enforces this");
    let t0 = std::time::Instant::now();

    let report = build_album(
        &photos,
        &cli.out,
        BuildOptions {
            title: cli.title.clone(),
            spreads: cli.spreads,
            trim,
            progress: Box::new(|line| eprintln!("{line}")),
        },
    )?;

    eprintln!(
        "done in {:.1?}: {} and {}",
        t0.elapsed(),
        report.album_json.display(),
        report.album_pdf.display()
    );
    Ok(())
}
