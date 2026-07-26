//! The pixeldelta command-line tool.

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Args, Parser, Subcommand};
use pixeldelta_core::{CompareOptions, Verdict};

use pixeldelta_cli::{
    ci, compare_files, exit_code, run_dirs, write_report, CiOptions, CompareRun, Storage,
};

#[derive(Parser)]
#[command(name = "pixeldelta", version, about = "Fast image comparison")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Compare two PNG images.
    Compare(CompareArgs),
    /// Compare two directories of PNG images.
    Run(RunArgs),
    /// Compare a directory against the snapshot stored for the baseline commit.
    Ci(CiArgs),
}

#[derive(Args)]
struct CompareArgs {
    /// Baseline image.
    base: PathBuf,
    /// Image compared against the baseline.
    head: PathBuf,
    /// Write the diff image to this path as PNG.
    #[arg(short, long)]
    output: Option<PathBuf>,
    /// Color delta a pixel must exceed to count, as a fraction in [0, 1].
    #[arg(long, default_value_t = 0.1)]
    threshold: f32,
    /// Count anti-aliasing differences instead of excluding them.
    #[arg(long)]
    no_antialiasing: bool,
}

#[derive(Args)]
struct RunArgs {
    /// Directory of baseline images.
    expected: PathBuf,
    /// Directory of images compared against the baseline.
    actual: PathBuf,
    /// Write the HTML report into this directory as index.html.
    #[arg(long)]
    report: Option<PathBuf>,
    /// Write the JSON report to this path.
    #[arg(long)]
    json: Option<PathBuf>,
    /// Write the JUnit XML report to this path.
    #[arg(long)]
    junit: Option<PathBuf>,
    /// Color delta a pixel must exceed to count, as a fraction in [0, 1].
    #[arg(long, default_value_t = 0.1)]
    threshold: f32,
    /// Count anti-aliasing differences instead of excluding them.
    #[arg(long)]
    no_antialiasing: bool,
}

#[derive(Args)]
struct CiArgs {
    /// Directory of images produced by this checkout.
    actual: PathBuf,
    /// Where snapshots are kept. A path without a scheme is a local directory.
    #[arg(long)]
    storage: String,
    /// Branch the baseline is looked for below.
    #[arg(long, default_value = "main")]
    base_branch: String,
    /// Repository the baseline is read from.
    #[arg(long, default_value = ".")]
    repo: PathBuf,
    /// How many commits the baseline search walks.
    #[arg(long, default_value_t = 50)]
    history_limit: usize,
    /// Write the HTML report into this directory as index.html.
    #[arg(long)]
    report: Option<PathBuf>,
    /// Write the JSON report to this path.
    #[arg(long)]
    json: Option<PathBuf>,
    /// Write the JUnit XML report to this path.
    #[arg(long)]
    junit: Option<PathBuf>,
    /// Append the notification body to this path, such as $GITHUB_STEP_SUMMARY.
    #[arg(long)]
    markdown: Option<PathBuf>,
    /// Color delta a pixel must exceed to count, as a fraction in [0, 1].
    #[arg(long, default_value_t = 0.1)]
    threshold: f32,
    /// Count anti-aliasing differences instead of excluding them.
    #[arg(long)]
    no_antialiasing: bool,
}

fn main() -> ExitCode {
    match Cli::parse().command {
        Command::Compare(args) => run_compare(args),
        Command::Run(args) => run(args),
        Command::Ci(args) => run_ci(args),
    }
}

fn run_ci(args: CiArgs) -> ExitCode {
    let storage = match Storage::parse(&args.storage) {
        Ok(storage) => storage,
        Err(error) => {
            eprintln!("error: {error}");
            return ExitCode::from(3);
        }
    };

    let opts = CiOptions {
        repo: &args.repo,
        actual: &args.actual,
        storage: &storage,
        base_branch: &args.base_branch,
        history_limit: args.history_limit,
        threshold: args.threshold,
        antialiasing: !args.no_antialiasing,
        report: args.report.as_deref(),
        json: args.json.as_deref(),
        junit: args.junit.as_deref(),
        markdown: args.markdown.as_deref(),
    };

    let run = match ci(&opts) {
        Ok(run) => run,
        Err(error) => {
            eprintln!("error: {error}");
            return ExitCode::from(error.exit_code() as u8);
        }
    };

    let Some(summary) = run.summary else {
        println!("no baseline: stored the snapshot for {}", run.head);
        return ExitCode::from(0);
    };

    println!(
        "{} against {}: {} changed, {} added, {} removed, {} size mismatch, {} matched",
        if summary.passed { "pass" } else { "fail" },
        run.baseline.as_deref().unwrap_or_default(),
        summary.changed,
        summary.added,
        summary.removed,
        summary.size_mismatch,
        summary.matched,
    );
    if let Some(url) = run.report_url {
        println!("report: {url}");
    }
    ExitCode::from(if summary.passed { 0 } else { 1 })
}

fn run(args: RunArgs) -> ExitCode {
    let report = match run_dirs(
        &args.expected,
        &args.actual,
        args.threshold,
        !args.no_antialiasing,
    ) {
        Ok(report) => report,
        Err(error) => {
            eprintln!("error: {error}");
            return ExitCode::from(error.exit_code() as u8);
        }
    };

    if let Err(error) = write_report(
        &report,
        args.report.as_deref(),
        args.json.as_deref(),
        args.junit.as_deref(),
    ) {
        eprintln!("error: {error}");
        return ExitCode::from(error.exit_code() as u8);
    }

    let summary = report.summary();
    println!(
        "{}: {} changed, {} added, {} removed, {} size mismatch, {} matched",
        if summary.passed { "pass" } else { "fail" },
        summary.changed,
        summary.added,
        summary.removed,
        summary.size_mismatch,
        summary.matched,
    );
    ExitCode::from(if summary.passed { 0 } else { 1 })
}

fn run_compare(args: CompareArgs) -> ExitCode {
    let opts = CompareOptions {
        threshold: args.threshold,
        detect_antialiasing: !args.no_antialiasing,
        ..Default::default()
    };

    match compare_files(&args.base, &args.head, &opts, args.output.as_deref()) {
        Ok(run) => {
            report(&run);
            ExitCode::from(exit_code(run.verdict) as u8)
        }
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::from(error.exit_code() as u8)
        }
    }
}

fn report(run: &CompareRun) {
    match run.verdict {
        Verdict::Match => println!("match"),
        Verdict::Differ => println!(
            "differ: {} pixels ({:.4}%)",
            run.diff_pixels,
            run.diff_ratio * 100.0
        ),
        Verdict::SizeMismatch => println!("size mismatch"),
    }
}
