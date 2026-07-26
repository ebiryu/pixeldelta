//! The pixeldelta command-line tool.

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Args, Parser, Subcommand};
use pixeldelta_core::{CompareOptions, Verdict};

use pixeldelta_cli::{
    ci, compare_files, exit_code, pull_request_number, run_dirs, write_report, CiOptions,
    CompareRun, GithubConfig, Notification, Storage,
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
    /// Post the notification body as a pull request comment, replacing the one
    /// an earlier run left. Reads GITHUB_TOKEN, GITHUB_REPOSITORY and
    /// GITHUB_API_URL from the environment.
    #[arg(long)]
    comment: bool,
    /// Pull request to comment on. Read from the workflow event when omitted.
    #[arg(long)]
    pr: Option<u64>,
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

    let github = if args.comment {
        match github_config(&args) {
            Ok(config) => Some(config),
            Err(message) => {
                eprintln!("error: {message}");
                return ExitCode::from(3);
            }
        }
    } else {
        None
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
        github: github.as_ref(),
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
    match run.comment {
        Some(Notification::Posted { url, .. }) => println!("comment: {url}"),
        Some(Notification::Refused) => {
            eprintln!("warning: the token may not comment on this repository")
        }
        None => {}
    }
    ExitCode::from(if summary.passed { 0 } else { 1 })
}

/// Collects what the comment route needs from the flags and the environment.
fn github_config(args: &CiArgs) -> Result<GithubConfig, String> {
    let variable = |name: &str| {
        std::env::var(name)
            .ok()
            .filter(|value| !value.is_empty())
            .ok_or_else(|| format!("{name} is not set, and --comment needs it"))
    };

    let pull_request = match args.pr {
        Some(number) => number,
        None => {
            let event = variable("GITHUB_EVENT_PATH")?;
            pull_request_number(std::path::Path::new(&event)).ok_or_else(|| {
                "this run is not on a pull request; pass --pr to name one".to_owned()
            })?
        }
    };

    Ok(GithubConfig {
        api_url: std::env::var("GITHUB_API_URL")
            .ok()
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "https://api.github.com".to_owned()),
        repository: variable("GITHUB_REPOSITORY")?,
        pull_request,
        token: variable("GITHUB_TOKEN")?,
    })
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
