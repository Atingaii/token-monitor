use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Context, Result};

const TASK_NAME: &str = "TokenMonitorUsageSync";

fn executable() -> Result<PathBuf> {
    std::env::current_exe().context("cannot locate token-monitor executable")
}

pub fn install(interval_minutes: u32) -> Result<String> {
    #[cfg(target_os = "windows")]
    { return install_windows(interval_minutes); }
    #[cfg(target_os = "macos")]
    { return install_macos(interval_minutes); }
    #[cfg(target_os = "linux")]
    { return install_linux(interval_minutes); }
    #[allow(unreachable_code)]
    bail!("automatic scheduling is not supported on this platform")
}

pub fn uninstall() -> Result<()> {
    #[cfg(target_os = "windows")]
    { return uninstall_windows(); }
    #[cfg(target_os = "macos")]
    { return uninstall_macos(); }
    #[cfg(target_os = "linux")]
    { return uninstall_linux(); }
    #[allow(unreachable_code)]
    Ok(())
}

fn run_ok(mut command: Command, description: &str) -> Result<()> {
    let output = command.output().with_context(|| format!("failed to run {description}"))?;
    if output.status.success() { return Ok(()); }
    bail!("{description} failed: {}", String::from_utf8_lossy(&output.stderr).trim())
}

#[cfg(target_os = "windows")]
fn install_windows(interval_minutes: u32) -> Result<String> {
    let exe = executable()?;
    let task = format!("\"{}\" sync --quiet", exe.display());
    let mut cmd = Command::new("schtasks.exe");
    cmd.args([
        "/Create", "/F", "/SC", "MINUTE", "/MO", &interval_minutes.to_string(),
        "/TN", TASK_NAME, "/TR", &task,
    ]);
    run_ok(cmd, "Windows Task Scheduler registration")?;
    let mut run = Command::new("schtasks.exe");
    run.args(["/Run", "/TN", TASK_NAME]);
    let _ = run.output();
    Ok(format!("Windows Task Scheduler every {interval_minutes} minutes"))
}

#[cfg(target_os = "windows")]
fn uninstall_windows() -> Result<()> {
    let mut cmd = Command::new("schtasks.exe");
    cmd.args(["/Delete", "/F", "/TN", TASK_NAME]);
    let _ = cmd.output();
    Ok(())
}

#[cfg(target_os = "macos")]
fn launch_agent_path() -> Result<PathBuf> {
    let home = dirs::home_dir().context("cannot determine home directory")?;
    Ok(home.join("Library/LaunchAgents/io.atingaii.token-monitor.plist"))
}

#[cfg(target_os = "macos")]
fn xml_escape(value: &str) -> String {
    value.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
        .replace('"', "&quot;").replace('\'', "&apos;")
}

#[cfg(target_os = "macos")]
fn mac_uid() -> Result<String> {
    let output = Command::new("id").arg("-u").output()?;
    if !output.status.success() { bail!("cannot determine macOS uid"); }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

#[cfg(target_os = "macos")]
fn install_macos(interval_minutes: u32) -> Result<String> {
    let exe = executable()?;
    let path = launch_agent_path()?;
    if let Some(parent) = path.parent() { fs::create_dir_all(parent)?; }
    let plist = format!(r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>Label</key><string>io.atingaii.token-monitor</string>
<key>ProgramArguments</key><array><string>{}</string><string>sync</string><string>--quiet</string></array>
<key>RunAtLoad</key><true/>
<key>StartInterval</key><integer>{}</integer>
<key>ProcessType</key><string>Background</string>
<key>LowPriorityIO</key><true/>
</dict></plist>
"#, xml_escape(&exe.to_string_lossy()), interval_minutes * 60);
    fs::write(&path, plist)?;
    let domain = format!("gui/{}", mac_uid()?);
    let mut bootout = Command::new("launchctl");
    bootout.args(["bootout", &domain, path.to_string_lossy().as_ref()]);
    let _ = bootout.output();
    let mut bootstrap = Command::new("launchctl");
    bootstrap.args(["bootstrap", &domain, path.to_string_lossy().as_ref()]);
    run_ok(bootstrap, "macOS launchd registration")?;
    Ok(format!("macOS launchd every {interval_minutes} minutes"))
}

#[cfg(target_os = "macos")]
fn uninstall_macos() -> Result<()> {
    let path = launch_agent_path()?;
    let domain = format!("gui/{}", mac_uid()?);
    let mut cmd = Command::new("launchctl");
    cmd.args(["bootout", &domain, path.to_string_lossy().as_ref()]);
    let _ = cmd.output();
    let _ = fs::remove_file(path);
    Ok(())
}

#[cfg(target_os = "linux")]
fn systemd_dir() -> Result<PathBuf> {
    Ok(dirs::config_dir().context("cannot determine config directory")?.join("systemd/user"))
}

#[cfg(target_os = "linux")]
fn install_linux(interval_minutes: u32) -> Result<String> {
    if Command::new("systemctl").arg("--version").output().is_ok() {
        let exe = executable()?;
        let dir = systemd_dir()?;
        fs::create_dir_all(&dir)?;
        let service = format!("[Unit]\nDescription=Token Monitor usage snapshot\n\n[Service]\nType=oneshot\nExecStart={} sync --quiet\nNice=10\nIOSchedulingClass=idle\n", systemd_escape(&exe));
        let timer = format!("[Unit]\nDescription=Periodic Token Monitor usage snapshot\n\n[Timer]\nOnBootSec=2min\nOnUnitActiveSec={}min\nRandomizedDelaySec=45\nPersistent=true\n\n[Install]\nWantedBy=timers.target\n", interval_minutes);
        fs::write(dir.join("token-monitor.service"), service)?;
        fs::write(dir.join("token-monitor.timer"), timer)?;
        let mut reload = Command::new("systemctl");
        reload.args(["--user", "daemon-reload"]);
        run_ok(reload, "systemd user daemon reload")?;
        let mut enable = Command::new("systemctl");
        enable.args(["--user", "enable", "--now", "token-monitor.timer"]);
        run_ok(enable, "systemd user timer registration")?;
        return Ok(format!("systemd user timer every {interval_minutes} minutes"));
    }
    install_cron(interval_minutes)
}

#[cfg(target_os = "linux")]
fn systemd_escape(path: &Path) -> String {
    let raw = path.to_string_lossy();
    if raw.contains([' ', '\t']) { format!("\"{}\"", raw.replace('"', "\\\"")) } else { raw.to_string() }
}

#[cfg(target_os = "linux")]
fn install_cron(interval_minutes: u32) -> Result<String> {
    let exe = executable()?;
    let existing = Command::new("crontab").arg("-l").output().ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
        .unwrap_or_default();
    let marker = "# token-monitor-usage-sync";
    let mut lines: Vec<String> = existing.lines().filter(|line| !line.contains(marker)).map(str::to_string).collect();
    let cadence = if interval_minutes >= 60 && interval_minutes % 60 == 0 {
        format!("0 */{} * * *", interval_minutes / 60)
    } else {
        format!("*/{} * * * *", interval_minutes.min(59))
    };
    lines.push(format!("{cadence} \"{}\" sync --quiet {marker}", exe.display()));
    let mut child = Command::new("crontab")
        .arg("-")
        .stdin(std::process::Stdio::piped())
        .spawn()
        .context("failed to start crontab")?;
    use std::io::Write;
    child.stdin.as_mut().unwrap().write_all(format!("{}\n", lines.join("\n")).as_bytes())?;
    let status = child.wait()?;
    if !status.success() { bail!("crontab registration failed"); }
    Ok(format!("cron approximately every {interval_minutes} minutes"))
}

#[cfg(target_os = "linux")]
fn uninstall_linux() -> Result<()> {
    let mut disable = Command::new("systemctl");
    disable.args(["--user", "disable", "--now", "token-monitor.timer"]);
    let _ = disable.output();
    if let Ok(dir) = systemd_dir() {
        let _ = fs::remove_file(dir.join("token-monitor.timer"));
        let _ = fs::remove_file(dir.join("token-monitor.service"));
    }
    let existing = Command::new("crontab").arg("-l").output().ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
        .unwrap_or_default();
    if !existing.is_empty() {
        let cleaned = existing.lines().filter(|line| !line.contains("# token-monitor-usage-sync")).collect::<Vec<_>>().join("\n");
        let mut child = Command::new("crontab").arg("-").stdin(std::process::Stdio::piped()).spawn()?;
        use std::io::Write;
        child.stdin.as_mut().unwrap().write_all(format!("{cleaned}\n").as_bytes())?;
        let _ = child.wait();
    }
    Ok(())
}
