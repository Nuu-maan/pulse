mod layout;
mod render;
mod repo;
mod theme;

use anyhow::{Context, Result, bail};
use chrono::{NaiveDate, Utc};
use clap::{Parser, ValueEnum};
use std::io::Write;
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "pulse",
    version,
    about = "Render a git repository's history as a self-contained animated SVG"
)]
struct Cli {
    #[arg(default_value = ".")]
    path: PathBuf,

    #[arg(
        short,
        long,
        default_value = "pulse.svg",
        help = "Output file, or - for stdout"
    )]
    out: String,

    #[arg(
        short = 'n',
        long,
        default_value_t = 140,
        help = "Most recent commits to include"
    )]
    max: usize,

    #[arg(long, help = "Only commits newer than this (30d, 6mo, 2y, 2026-01-01)")]
    since: Option<String>,

    #[arg(long, help = "Only commits older than this, same formats as --since")]
    until: Option<String>,

    #[arg(long, help = "Start from this revision instead of HEAD")]
    rev: Option<String>,

    #[arg(long, help = "Include every local branch, not just HEAD")]
    all: bool,

    #[arg(long, help = "Follow only the first parent, hiding merged-in commits")]
    first_parent: bool,

    #[arg(long, value_enum, default_value_t = ThemeArg::Auto)]
    theme: ThemeArg,

    #[arg(long, default_value_t = 1200.0, help = "Target width in pixels")]
    width: f64,

    #[arg(
        short,
        long,
        default_value_t = 9.0,
        help = "Animation length in seconds"
    )]
    duration: f64,

    #[arg(long, help = "Play the animation once instead of looping")]
    once: bool,

    #[arg(long = "static", help = "Emit a still frame with no animation")]
    no_anim: bool,

    #[arg(long, value_enum, default_value_t = ColorArg::Lane)]
    color_by: ColorArg,

    #[arg(long, help = "Drop the churn waveform and skip reading diffs")]
    no_pulse: bool,

    #[arg(long, help = "Override the title shown in the header")]
    title: Option<String>,

    #[arg(long, help = "Drop the header block")]
    no_header: bool,

    #[arg(long, help = "Drop branch and tag labels")]
    no_labels: bool,
}

#[derive(Clone, Copy, ValueEnum)]
enum ThemeArg {
    Auto,
    Light,
    Dark,
}

#[derive(Clone, Copy, ValueEnum)]
enum ColorArg {
    Lane,
    Author,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    if cli.max == 0 {
        bail!("--max must be at least 1");
    }
    if cli.duration <= 0.0 {
        bail!("--duration must be positive");
    }

    let since = cli.since.as_deref().map(parse_time).transpose()?;
    let until = cli.until.as_deref().map(parse_time).transpose()?;
    if let (Some(a), Some(b)) = (since, until)
        && a > b
    {
        bail!("--since is newer than --until, so no commit can match");
    }
    let history = repo::load(
        &cli.path,
        &repo::Query {
            rev: cli.rev.as_deref(),
            all: cli.all,
            first_parent: cli.first_parent,
            max: cli.max,
            since,
            until,
            stats: !cli.no_pulse,
        },
    )?;
    let layout = layout::compute(&history.commits);

    let opts = render::Options {
        width: cli.width.max(320.0),
        theme: match cli.theme {
            ThemeArg::Auto => render::Theme::Auto,
            ThemeArg::Light => render::Theme::Light,
            ThemeArg::Dark => render::Theme::Dark,
        },
        duration: cli.duration,
        once: cli.once,
        animate: !cli.no_anim,
        color_by: match cli.color_by {
            ColorArg::Lane => render::ColorBy::Lane,
            ColorArg::Author => render::ColorBy::Author,
        },
        pulse: !cli.no_pulse,
        title: cli.title,
        header: !cli.no_header,
        labels: !cli.no_labels,
    };

    let out = render::svg(&history, &layout, &opts);

    if cli.out == "-" {
        std::io::stdout().write_all(out.svg.as_bytes())?;
        return Ok(());
    }

    if let Some(parent) = PathBuf::from(&cli.out).parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("cannot create {}", parent.display()))?;
    }
    std::fs::write(&cli.out, &out.svg).with_context(|| format!("cannot write {}", cli.out))?;
    eprintln!(
        "{} · {} commit{} · {} lane{} · {:.0}x{:.0} px · {:.1} kB → {}",
        history.name,
        history.commits.len(),
        if history.commits.len() == 1 { "" } else { "s" },
        layout.lanes,
        if layout.lanes == 1 { "" } else { "s" },
        out.width,
        out.height,
        out.svg.len() as f64 / 1024.0,
        cli.out
    );
    if out.width > cli.width + 1.0 {
        eprintln!(
            "note: {} commits need {:.0}px to stay legible; use -n to fit {:.0}px",
            history.commits.len(),
            out.width,
            cli.width
        );
    }
    Ok(())
}

fn parse_time(s: &str) -> Result<i64> {
    if let Ok(d) = NaiveDate::parse_from_str(s, "%Y-%m-%d") {
        return Ok(d
            .and_hms_opt(0, 0, 0)
            .context("invalid date")?
            .and_utc()
            .timestamp());
    }
    let split = s
        .find(|c: char| c.is_ascii_alphabetic())
        .with_context(|| format!("cannot parse a time from `{s}`"))?;
    let (num, unit) = s.split_at(split);
    let n: i64 = num
        .trim()
        .parse()
        .with_context(|| format!("cannot parse a time from `{s}`"))?;
    let secs = match unit.trim().to_lowercase().as_str() {
        "d" | "day" | "days" => 86_400,
        "w" | "week" | "weeks" => 604_800,
        "mo" | "month" | "months" => 2_629_800,
        "y" | "year" | "years" => 31_557_600,
        other => bail!("unknown time unit `{other}` (use d, w, mo, y or YYYY-MM-DD)"),
    };
    Ok(Utc::now().timestamp() - n * secs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn absolute_dates_parse_as_midnight_utc() {
        let t = parse_time("2026-01-15").unwrap();
        let d = chrono::DateTime::from_timestamp(t, 0).unwrap();
        assert_eq!(d.format("%Y-%m-%d %H:%M").to_string(), "2026-01-15 00:00");
    }

    #[test]
    fn relative_spans_count_backwards_from_now() {
        let now = Utc::now().timestamp();
        let day = parse_time("1d").unwrap();
        assert!((now - day - 86_400).abs() <= 2, "one day back");

        let week = parse_time("2w").unwrap();
        assert!((now - week - 2 * 604_800).abs() <= 2, "two weeks back");

        assert!(parse_time("6mo").unwrap() < parse_time("1mo").unwrap());
        assert!(parse_time("1y").unwrap() < parse_time("6mo").unwrap());
    }

    #[test]
    fn long_unit_names_are_accepted() {
        let a = parse_time("3days").unwrap();
        let b = parse_time("3d").unwrap();
        assert!((a - b).abs() <= 2);
    }

    #[test]
    fn nonsense_is_rejected_with_a_message() {
        for bad in ["", "soon", "12", "3x", "-d", "2026-13-45"] {
            let err = parse_time(bad).unwrap_err().to_string();
            assert!(!err.is_empty(), "expected an error for {bad:?}");
        }
    }
}
