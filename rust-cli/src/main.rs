mod collector;
mod config;
mod crypto;
mod github;
mod model;
mod scheduler;

use std::fs;
use std::io::{self, Write};
use std::process::Command;

use anyhow::{bail, Context, Result};
use chrono::{Duration, Local};
use clap::{Parser, Subcommand};

use crate::config::Config;
use crate::github::GithubClient;
use crate::model::DeviceInfo;

#[derive(Parser)]
#[command(name = "token-monitor", version, about = "Zero-server, cross-device AI token usage analytics")]
struct Cli {
    #[command(subcommand)]
    command: CommandKind,
}

#[derive(Subcommand)]
enum CommandKind {
    /// First device: create a workspace. Other devices: use --join.
    Setup {
        /// Public GitHub repository used for encrypted snapshots and Pages (OWNER/REPO).
        #[arg(long)]
        repo: Option<String>,
        /// Join code printed by another configured device.
        #[arg(long, conflicts_with = "repo")]
        join: Option<String>,
        /// Fine-grained GitHub PAT. Omit to use GITHUB_TOKEN, `gh auth token`, or a hidden prompt.
        #[arg(long)]
        token: Option<String>,
        /// Friendly device name. Defaults to the hostname.
        #[arg(long)]
        device: Option<String>,
        /// Snapshot cadence. No process stays resident between runs.
        #[arg(long, default_value_t = 15)]
        interval: u32,
        /// Configure without installing the OS-native timer.
        #[arg(long)]
        no_schedule: bool,
    },
    /// Collect an incremental local snapshot and upload it to this device's GitHub data branch.
    Sync {
        /// Re-scan the complete local history instead of the rolling incremental window.
        #[arg(long)]
        full: bool,
        #[arg(long)]
        quiet: bool,
    },
    /// Show local configuration, last sync and aggregate usage.
    Status,
    /// Print the join code and browser dashboard URL for this workspace.
    Join,
    /// Print only the browser dashboard URL.
    Dashboard,
    /// Remove the OS-native timer. Optionally remove this device's remote snapshot and local data.
    Uninstall {
        #[arg(long)]
        remove_remote: bool,
        #[arg(long)]
        purge: bool,
    },
}

fn prompt_line(label: &str) -> Result<String> {
    print!("{label}");
    io::stdout().flush()?;
    let mut value = String::new();
    io::stdin().read_line(&mut value)?;
    Ok(value.trim().to_string())
}

fn resolve_token(explicit: Option<String>) -> Result<String> {
    if let Some(token) = explicit.filter(|value| !value.trim().is_empty()) {
        return Ok(token.trim().to_string());
    }
    for key in ["TOKEN_MONITOR_GITHUB_TOKEN", "GITHUB_TOKEN", "GH_TOKEN"] {
        if let Ok(token) = std::env::var(key) {
            if !token.trim().is_empty() { return Ok(token.trim().to_string()); }
        }
    }
    if let Ok(output) = Command::new("gh").args(["auth", "token"]).output() {
        if output.status.success() {
            let token = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !token.is_empty() { return Ok(token); }
        }
    }
    let token = rpassword::prompt_password("GitHub fine-grained PAT (input hidden): ")?;
    if token.trim().is_empty() { bail!("a GitHub token with Contents: read/write is required"); }
    Ok(token.trim().to_string())
}

fn device_info(config: &Config) -> DeviceInfo {
    DeviceInfo {
        id: config.device_id.clone(),
        name: config.device_name.clone(),
        platform: std::env::consts::OS.to_string(),
        arch: std::env::consts::ARCH.to_string(),
        hostname: config::default_device_name(),
        app_version: env!("CARGO_PKG_VERSION").to_string(),
    }
}

fn run_sync(full: bool, quiet: bool) -> Result<()> {
    let config = config::load()?;
    let previous = config::read_cached_ledger()?;
    let (ledger, mode) = if full || previous.is_none() {
        (collector::collect(device_info(&config), None)?, "full")
    } else {
        // Re-read a small overlap so late-written sessions and timezone-edge rows
        // are corrected without repeatedly scanning years of history.
        let since = (Local::now().date_naive() - Duration::days(2)).format("%Y-%m-%d").to_string();
        let partial = collector::collect(device_info(&config), Some(since.clone()))?;
        (collector::merge_incremental(previous.unwrap(), partial, &since), "incremental")
    };

    config::write_cached_ledger(&ledger)?;
    let envelope = crypto::encrypt_ledger(&ledger, &config.dashboard_key)?;
    let github = GithubClient::new(config.repo.clone(), config.github_token.clone())?;
    let branch = github.replace_snapshot(&config.device_id, &envelope)?;

    if !quiet {
        println!("Synced {} ({mode})", config.device_name);
        println!("  Branch: {branch}");
        println!("  Rows: {}", ledger.rows.len());
        println!("  Tokens: {}", ledger.totals.total_tokens());
        println!("  API-equivalent cost: ${:.2}", ledger.totals.cost_usd);
        println!("  Scan: {} ms", ledger.scan_ms);
    }
    Ok(())
}

fn setup(
    repo: Option<String>,
    join: Option<String>,
    token: Option<String>,
    device: Option<String>,
    interval: u32,
    no_schedule: bool,
) -> Result<()> {
    let token = resolve_token(token)?;
    let config = if let Some(code) = join {
        config::from_join(&code, token, device)?
    } else {
        let repo = match repo {
            Some(repo) => repo,
            None => prompt_line("GitHub repository (OWNER/REPO): ")?,
        };
        if repo.trim().is_empty() { bail!("repository is required"); }
        config::new_config(&repo, token, device, interval)?
    };

    let github = GithubClient::new(config.repo.clone(), config.github_token.clone())?;
    github.validate().context("cannot use the selected GitHub repository")?;
    config::save(&config)?;

    println!("Collecting the first local snapshot...");
    run_sync(true, false)?;

    if !no_schedule {
        match scheduler::install(config.interval_minutes) {
            Ok(description) => println!("Automatic sync: {description}"),
            Err(error) => {
                eprintln!("Automatic timer could not be installed: {error}");
                eprintln!("You can still run `token-monitor sync` manually.");
            }
        }
    }

    println!();
    println!("Dashboard: {}", config::dashboard_url(&config));
    println!("Join code (share only with your own devices):");
    println!("{}", config::join_code(&config)?);
    println!();
    println!("No monitor process remains resident; the OS timer launches a short sync only when needed.");
    Ok(())
}

fn status() -> Result<()> {
    let config = config::load()?;
    println!("Token Monitor {}", env!("CARGO_PKG_VERSION"));
    println!("Device: {} ({}/{})", config.device_name, std::env::consts::OS, std::env::consts::ARCH);
    println!("Repository: {}", config.repo);
    println!("Snapshot branch: {}", GithubClient::snapshot_branch(&config.device_id));
    println!("Interval: {} minutes", config.interval_minutes);
    if let Some(ledger) = config::read_cached_ledger()? {
        println!("Last sync: {}", ledger.generated_at);
        println!("Tokens: {}", ledger.totals.total_tokens());
        println!("API-equivalent cost: ${:.2}", ledger.totals.cost_usd);
        println!("Rows: {}", ledger.rows.len());
        println!("Last scan CPU work: {} ms wall time", ledger.scan_ms);
    } else {
        println!("Last sync: never");
    }
    println!("Dashboard: {}", config::dashboard_url(&config));
    Ok(())
}

fn uninstall(remove_remote: bool, purge: bool) -> Result<()> {
    let config = config::load().ok();
    scheduler::uninstall()?;
    if remove_remote {
        if let Some(ref config) = config {
            let github = GithubClient::new(config.repo.clone(), config.github_token.clone())?;
            github.remove_snapshot_branch(&config.device_id)?;
        }
    }
    if purge {
        if let Ok(dir) = config::config_dir() { let _ = fs::remove_dir_all(dir); }
    }
    println!("Automatic sync removed{}.", if purge { " and local configuration purged" } else { "" });
    Ok(())
}

fn main() {
    if let Err(error) = real_main() {
        eprintln!("error: {error:#}");
        std::process::exit(1);
    }
}

fn real_main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        CommandKind::Setup { repo, join, token, device, interval, no_schedule } => {
            setup(repo, join, token, device, interval, no_schedule)
        }
        CommandKind::Sync { full, quiet } => run_sync(full, quiet),
        CommandKind::Status => status(),
        CommandKind::Join => {
            let config = config::load()?;
            println!("{}", config::join_code(&config)?);
            println!("{}", config::dashboard_url(&config));
            Ok(())
        }
        CommandKind::Dashboard => {
            println!("{}", config::dashboard_url(&config::load()?));
            Ok(())
        }
        CommandKind::Uninstall { remove_remote, purge } => uninstall(remove_remote, purge),
    }
}
