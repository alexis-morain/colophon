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
    #[arg(required_unless_present_any = ["formats", "profils", "profils_json", "dump_geometry", "print", "cover", "audit", "reprise", "prevol", "sheets"])]
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

    /// Composition pace: aeree, equilibree, dense. Same photos and the same
    /// rules; how many of them land on a spread, and how many pages the
    /// album ends up with.
    #[arg(long, default_value = "equilibree", value_name = "PACE")]
    densite: String,

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

    /// Lint the album already built in --out: count the defect classes
    /// (visage coupé, orientation trahie, doublons sur une planche…) and
    /// print the JSON report. Exits non-zero when a counter passes son seuil.
    #[arg(long)]
    audit: bool,

    /// Measure how much of the composer's proposal was corrected by hand:
    /// compares album.json against the album.origin.json written at the
    /// first build. Exits non-zero past 30 % of spreads touched.
    #[arg(long)]
    reprise: bool,

    /// Render album-cover.pdf: the flat cover sheet for --profil, back cover
    /// then spine then front, at 300 dpi. The spine and the bleed both come
    /// from the profile, so the same album gives a different sheet at each
    /// supplier.
    #[arg(long)]
    cover: bool,

    /// Preflight the album already built in --out against a printer profile:
    /// resolution, pagination, bleed, colour space, fonts, safe zone. Prints
    /// the report plus the spec sheet. Exits non-zero on a blocking defect.
    #[arg(long)]
    prevol: bool,

    /// Printer profile the export and the preflight read: cloudprinter,
    /// prodigi, lulu, generique.
    #[arg(long, default_value = "cloudprinter", value_name = "ID")]
    profil: String,

    /// List the printer profiles and exit
    #[arg(long)]
    profils: bool,

    /// The same profiles as JSON, whole. Feeds the dev album server, which
    /// stands in for the Tauri commands when the destination screen is
    /// worked on in a browser.
    #[arg(long, hide = true)]
    profils_json: bool,

    /// Write one PDF per template into DIR, slots filled with the check
    /// palette. Feeds the PDF → PNG raster non-regression.
    #[arg(long, value_name = "DIR", hide = true)]
    sheets: Option<PathBuf>,
}

/// Built once at startup so `--help` can show the format table.
static FORMAT_HELP: std::sync::LazyLock<String> = std::sync::LazyLock::new(|| {
    format!("Formats de page :\n{}\n\nOu une taille libre, par exemple --format 240x180.", format::help())
});

/// The printer profile behind `--profil`, or a message naming the way out.
fn profil(id: &str) -> Result<&'static colophon_core::printer::PrinterProfile> {
    colophon_core::printer::PrinterProfile::par_id(id)
        .ok_or_else(|| anyhow::anyhow!("profil inconnu : {id} (voir --profils)"))
}

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

    if let Some(dir) = &cli.sheets {
        let album = Album::new("gabarits", std::path::Path::new("."), trim);
        let files = pdf::render_template_sheets(&album, dir)?;
        eprintln!("{} gabarits rendus dans {}", files.len(), dir.display());
        return Ok(());
    }

    if cli.audit {
        let report = colophon_core::audit::audit(&cli.out)?;
        println!("{}", serde_json::to_string_pretty(&report)?);
        if !report.ok {
            std::process::exit(1);
        }
        return Ok(());
    }

    if cli.profils {
        for p in colophon_core::printer::PrinterProfile::tous() {
            println!("{:<14} {}", p.id, p.nom);
        }
        return Ok(());
    }

    if cli.profils_json {
        println!(
            "{}",
            serde_json::to_string(colophon_core::printer::PrinterProfile::tous())?
        );
        return Ok(());
    }

    if cli.prevol {
        let report = colophon_core::prevol::prevol(&cli.out, profil(&cli.profil)?)?;
        println!("{}", serde_json::to_string_pretty(&report)?);
        if !report.ok {
            std::process::exit(1);
        }
        return Ok(());
    }

    if cli.reprise {
        let report = colophon_core::reprise::reprise(&cli.out)?;
        println!("{}", serde_json::to_string_pretty(&report)?);
        if !report.ok {
            std::process::exit(1);
        }
        return Ok(());
    }

    if cli.cover {
        let profil = profil(&cli.profil)?;
        let t0 = std::time::Instant::now();
        let out = colophon_core::cover::render_cover_pdf(
            &cli.out,
            profil,
            &cli.out.join("album-cover.pdf"),
        )?;
        // The sheet's own dimensions, printed rather than left to be guessed:
        // this is the number that gets checked against the supplier's template.
        let album: Album = serde_json::from_str(&std::fs::read_to_string(
            cli.out.join("album.json"),
        )?)?;
        let g = colophon_core::cover::geometry(&album, profil);
        eprintln!(
            "done in {:.1?}: {} — {:.1} × {:.1} mm, dos {:.1} mm, fond perdu {:.1} mm",
            t0.elapsed(),
            out.display(),
            g.media_w,
            g.media_h,
            g.spine_mm(),
            g.bleed_ext
        );
        return Ok(());
    }

    if cli.print {
        let t0 = std::time::Instant::now();
        let out = colophon_core::render_print_pdf(
            &cli.out,
            &cli.out.join("album-print.pdf"),
            &|line| eprintln!("{line}"),
            &|| false,
        )?;
        eprintln!("done in {:.1?}: {}", t0.elapsed(), out.display());
        return Ok(());
    }

    let densite = colophon_core::layout::Densite::par_id(&cli.densite).ok_or_else(|| {
        anyhow::anyhow!(
            "densité inconnue : {} (aeree, equilibree, dense)",
            cli.densite
        )
    })?;
    let photos = cli.photos.clone().expect("clap enforces this");
    let t0 = std::time::Instant::now();

    let report = build_album(
        &photos,
        &cli.out,
        BuildOptions {
            title: cli.title.clone(),
            spreads: cli.spreads,
            trim,
            densite,
            progress: Box::new(|line| eprintln!("{line}")),
            ..Default::default()
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
