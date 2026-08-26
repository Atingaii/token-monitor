mod codex_tier;
mod collector;
mod config;
mod crypto;
mod evidence;
mod github;
mod model;
mod provider;
mod scheduler;

use std::fs;
use std::process::Command;

use anyhow::{bail, Context, Result};
use chrono::{Duration, Local};
use clap::{Parser, Subcommand};

use crate::config::Config;
use crate::github::GithubClient;
use crate::model::{DeviceInfo, PublicLedger};

#[derive(Parser)]
#[command(name = "token-monitor", version, about = "Zero-server, cross-device AI coding token analytics")]
struct Cli {
    #[command(subcommand)]
    command: CommandKind,
}

#[derive(Subcommand)]
enum CommandKind {
    /// Configure the first device. The user's project fork is discovered/created automatically.
    Setup {
        /// Advanced override for a renamed or organization-owned fork.
        #[arg(long)]
        repo: Option<String>,
        /// GitHub credential. Usually omitted: env vars or an authenticated `gh` are auto-detected.
        #[arg(long)]
        token: Option<String>,
        /// Friendly local device name. Public dashboard snapshots use an anonymous device label.
        #[arg(long)]
        device: Option<String>,
        /// Snapshot cadence in minutes. No process stays resident between runs.
        #[arg(long, default_value_t = 15)]
        interval: u32,
        /// Configure without installing the native OS timer.
        #[arg(long)]
        no_schedule: bool,
    },
    /// Add this machine to an existing Token Monitor workspace using its pair code.
    Join {
        /// Pair code printed by `setup` or `invite` on an existing device.
        code: String,
        #[arg(long)]
        token: Option<String>,
        #[arg(long)]
        device: Option<String>,
        #[arg(long)]
        no_schedule: bool,
    },
    /// Incrementally collect local usage and replace this device's GitHub snapshot.
    Sync {
        #[arg(long)]
        full: bool,
        #[arg(long)]
        quiet: bool,
    },
    /// Show local configuration, last sync and aggregate usage.
    Status,
    /// List every AI coding client supported by the embedded Tokscale scanner.
    Clients,
    /// Print a copy-paste command for adding another device.
    Invite,
    /// Print only the public dashboard URL.
    Dashboard,
    /// Remove the native timer; optionally remove the remote snapshot and local data.
    Uninstall {
        #[arg(long)]
        remove_remote: bool,
        #[arg(long)]
        purge: bool,
    },
}

fn resolve_token(explicit: Option<String>) -> Result<String> {
    if let Some(token) = explicit.filter(|value| !value.trim().is_empty()) {
        return Ok(token.trim().to_string());
    }
    for key in ["TOKEN_MONITOR_GITHUB_TOKEN", "GITHUB_TOKEN", "GH_TOKEN"] {
        if let Ok(token) = std::env::var(key) {
            if !token.trim().is_empty() {
                return Ok(token.trim().to_string());
            }
        }
    }
    if let Ok(output) = Command::new("gh").args(["auth", "token"]).output() {
        if output.status.success() {
            let token = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !token.is_empty() {
                return Ok(token);
            }
        }
    }
    let token = rpassword::prompt_password("GitHub token (input hidden; one-time setup only): ")?;
    if token.trim().is_empty() {
        bail!("GitHub authentication is required. Sign in with `gh auth login`, set GITHUB_TOKEN, or paste a token when prompted")
    }
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

fn print_costs(ledger: &crate::model::Ledger) {
    if ledger.rows.iter().any(|row| row.plan_cost_available) {
        println!("  Plan-equivalent cost: ${:.2}", ledger.totals.plan_cost_usd);
    }
    println!("  API-equivalent cost: ${:.2}", ledger.totals.cost_usd);
}

fn run_sync(full: bool, quiet: bool) -> Result<()> {
    let config = config::load()?;
    let previous = config::read_cached_ledger()?;
    let previous_for_compare = previous.clone();

    let (ledger, mode) = if full || previous.is_none() {
        (collector::collect(device_info(&config), None)?, "full")
    } else {
        let since = (Local::now().date_naive() - Duration::days(2))
            .format("%Y-%m-%d")
            .to_string();
        let partial = collector::collect(device_info(&config), Some(since.clone()))?;
        (
            collector::merge_incremental(previous.expect("checked above"), partial, &since),
            "incremental",
        )
    };

    config::write_cached_ledger(&ledger)?;
    if previous_for_compare
        .as_ref()
        .is_some_and(|previous| collector::same_accounting(previous, &ledger))
    {
        if !quiet {
            println!("No usage changes on {}; GitHub snapshot unchanged.", config.device_name);
            println!("  Scan: {} ms", ledger.scan_ms);
        }
        return Ok(());
    }

    let envelope = crypto::encrypt_ledger(&ledger, &config.dashboard_key)?;
    let public_ledger = PublicLedger::from_ledger(&ledger, &envelope.device_hash);
    let github = GithubClient::new(config.repo.clone(), config.github_token.clone())?;
    let branch = github.replace_snapshot(&config.device_id, &envelope, &public_ledger)?;
    if !quiet {
        println!("Synced {} ({mode})", config.device_name);
        println!("  Branch: {branch}");
        println!("  Rows: {}", ledger.rows.len());
        println!("  Tokens: {}", ledger.totals.total_tokens());
        print_costs(&ledger);
        println!("  Scan: {} ms", ledger.scan_ms);
        println!("  Public dashboard aggregate: enabled (de-identified)");
    }
    Ok(())
}

fn finish_onboarding(config: &Config, no_schedule: bool) -> Result<()> {
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
    println!("Dashboard (no key required):");
    println!("{}", config::dashboard_url(config));
    println!();
    println!("Add another device with this single command:");
    println!("token-monitor join '{}'", config::join_code(config)?);
    println!();
    println!("No Token Monitor process stays resident between scheduled syncs.");
    Ok(())
}

fn setup(
    repo: Option<String>,
    token: Option<String>,
    device: Option<String>,
    interval: u32,
    no_schedule: bool,
) -> Result<()> {
    let token = resolve_token(token)?;
    let repo = match repo {
        Some(repo) => config::normalize_repo(&repo)?,
        None => {
            println!("Finding your Token Monitor fork on GitHub...");
            github::ensure_user_fork(&token)
                .context("could not automatically prepare your Token Monitor fork")?
        }
    };
    let config = config::new_config(&repo, token, device, interval)?;
    GithubClient::new(config.repo.clone(), config.github_token.clone())?
        .validate()
        .context("cannot use the selected Token Monitor fork")?;
    config::save(&config)?;
    println!("Workspace: {}", config.repo);
    finish_onboarding(&config, no_schedule)
}

fn join(
    code: String,
    token: Option<String>,
    device: Option<String>,
    no_schedule: bool,
) -> Result<()> {
    let token = resolve_token(token)?;
    let config = config::from_join(&code, token, device)?;
    GithubClient::new(config.repo.clone(), config.github_token.clone())?
        .validate()
        .context("cannot access the Token Monitor fork encoded in this pair code")?;
    config::save(&config)?;
    println!("Joined workspace: {}", config.repo);
    finish_onboarding(&config, no_schedule)
}

fn status() -> Result<()> {
    let config = config::load()?;
    println!("Token Monitor {}", env!("CARGO_PKG_VERSION"));
    println!(
        "Device: {} ({}/{})",
        config.device_name,
        std::env::consts::OS,
        std::env::consts::ARCH
    );
    println!("Workspace fork: {}", config.repo);
    println!("Snapshot branch: {}", GithubClient::snapshot_branch(&config.device_id));
    println!("Interval: {} minutes", config.interval_minutes);
    if let Some(ledger) = config::read_cached_ledger()? {
        println!("Last scan: {}", ledger.generated_at);
        println!("Tokens: {}", ledger.totals.total_tokens());
        if ledger.rows.iter().any(|row| row.plan_cost_available) {
            println!("Plan-equivalent cost: ${:.2}", ledger.totals.plan_cost_usd);
        }
        println!("API-equivalent cost: ${:.2}", ledger.totals.cost_usd);
        println!("Rows: {}", ledger.rows.len());
        println!("Scan work: {} ms wall time", ledger.scan_ms);
    } else {
        println!("Last scan: never");
    }
    println!("Dashboard: {}", config::dashboard_url(&config));
    Ok(())
}

fn uninstall(remove_remote: bool, purge: bool) -> Result<()> {
    let config = config::load().ok();
    scheduler::uninstall()?;
    if remove_remote {
        if let Some(ref config) = config {
            GithubClient::new(config.repo.clone(), config.github_token.clone())?
                .remove_snapshot_branch(&config.device_id)?;
        }
    }
    if purge {
        if let Ok(dir) = config::config_dir() {
            let _ = fs::remove_dir_all(dir);
        }
    }
    println!(
        "Automatic sync removed{}.",
        if purge { " and local configuration purged" } else { "" }
    );
    Ok(())
}

fn real_main() -> Result<()> {
    match Cli::parse().command {
        CommandKind::Setup { repo, token, device, interval, no_schedule } => {
            setup(repo, token, device, interval, no_schedule)
        }
        CommandKind::Join { code, token, device, no_schedule } => {
            join(code, token, device, no_schedule)
        }
        CommandKind::Sync { full, quiet } => run_sync(full, quiet),
        CommandKind::Status => status(),
        CommandKind::Clients => {
            for client in collector::supported_clients() {
                println!("{client}");
            }
            Ok(())
        }
        CommandKind::Invite => {
            let config = config::load()?;
            println!("token-monitor join '{}'", config::join_code(&config)?);
            Ok(())
        }
        CommandKind::Dashboard => {
            println!("{}", config::dashboard_url(&config::load()?));
            Ok(())
        }
        CommandKind::Uninstall { remove_remote, purge } => uninstall(remove_remote, purge),
    }
}

fn main() {
    if let Err(error) = real_main() {
        eprintln!("error: {error:#}");
        std::process::exit(1);
    }
}
