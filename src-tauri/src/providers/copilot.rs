use super::{MetricLine, MetricFormat};
use std::process::Command;

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

const USAGE_URL: &str = "https://api.github.com/copilot_internal/user";
#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x08000000;

/// Get GitHub CLI hosts.yml path
fn get_gh_hosts_path() -> Option<std::path::PathBuf> {
    if cfg!(target_os = "windows") {
        std::env::var("APPDATA").ok().map(|d| {
            std::path::PathBuf::from(d).join("GitHub CLI").join("hosts.yml")
        })
    } else {
        dirs::config_dir().map(|d| d.join("gh").join("hosts.yml"))
    }
}

fn get_gh_executable_path() -> std::path::PathBuf {
    #[cfg(target_os = "windows")]
    {
        let candidates = [
            std::path::PathBuf::from(r"C:\Program Files\GitHub CLI\gh.exe"),
            std::path::PathBuf::from(r"C:\Program Files (x86)\GitHub CLI\gh.exe"),
            std::env::var_os("LOCALAPPDATA")
                .map(std::path::PathBuf::from)
                .map(|p| p.join("Programs").join("GitHub CLI").join("gh.exe"))
                .unwrap_or_default(),
        ];

        for candidate in candidates {
            if !candidate.as_os_str().is_empty() && candidate.exists() {
                return candidate;
            }
        }
    }

    std::path::PathBuf::from("gh")
}

fn load_token() -> Result<String, String> {
    let hosts_path = get_gh_hosts_path()
        .ok_or("Cannot determine GitHub CLI config path")?;

    if hosts_path.exists() {
        let content = std::fs::read_to_string(&hosts_path)
            .map_err(|e| format!("Failed to read gh config: {}", e))?;

        // Older gh stores oauth_token directly in hosts.yml.
        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("oauth_token:") {
                let token = trimmed.strip_prefix("oauth_token:").unwrap().trim();
                if !token.is_empty() {
                    return Ok(token.to_string());
                }
            }
        }
    }

    load_token_from_gh_cli()
}

fn load_token_from_gh_cli() -> Result<String, String> {
    let mut command = Command::new(get_gh_executable_path());
    command.args(["auth", "token", "--hostname", "github.com"]);

    #[cfg(target_os = "windows")]
    command.creation_flags(CREATE_NO_WINDOW);

    let output = command.output().map_err(|_| {
        "No GitHub token found. Install GitHub CLI and run `gh auth login` first.".to_string()
    })?;

    if !output.status.success() {
        return Err("No GitHub token found. Run `gh auth login` first.".into());
    }

    let token = String::from_utf8(output.stdout)
        .map_err(|e| format!("Invalid gh auth token output: {}", e))?
        .trim()
        .to_string();

    if token.is_empty() {
        return Err("No GitHub token found. Run `gh auth login` first.".into());
    }

    Ok(token)
}

fn fetch_usage(token: &str) -> Result<serde_json::Value, String> {
    let client = reqwest::blocking::Client::new();
    let resp = client
        .get(USAGE_URL)
        .header("Authorization", format!("token {}", token))
        .header("Accept", "application/json")
        .header("Editor-Version", "vscode/1.96.2")
        .header("Editor-Plugin-Version", "copilot-chat/0.26.7")
        .header("User-Agent", "GitHubCopilotChat/0.26.7")
        .header("X-Github-Api-Version", "2025-04-01")
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .map_err(|e| format!("Network error: {}", e))?;

    let status = resp.status().as_u16();
    if status == 401 || status == 403 {
        return Err("Token invalid. Run `gh auth login` to re-authenticate.".into());
    }
    if status < 200 || status >= 300 {
        return Err(format!("Usage request failed (HTTP {})", status));
    }

    resp.json::<serde_json::Value>()
        .map_err(|e| format!("Invalid response: {}", e))
}

pub fn probe() -> Result<(Option<String>, Vec<MetricLine>), String> {
    let token = load_token()?;
    let data = fetch_usage(&token)?;

    let mut lines = Vec::new();
    let plan = data.get("copilot_plan")
        .and_then(|v| v.as_str())
        .map(capitalize);

    // Paid tier: quota_snapshots
    if let Some(snapshots) = data.get("quota_snapshots") {
        if let Some(premium) = snapshots.get("premium_interactions") {
            if let Some(remaining) = premium.get("percent_remaining").and_then(|v| v.as_f64()) {
                let used = (100.0 - remaining).max(0.0).min(100.0);
                let resets_at = data.get("quota_reset_date")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                lines.push(MetricLine::Progress {
                    label: "Premium".into(),
                    used,
                    limit: 100.0,
                    format: MetricFormat { kind: "percent".into(), suffix: None },
                    resets_at,
                });
            }
        }

        if let Some(chat) = snapshots.get("chat") {
            if let Some(remaining) = chat.get("percent_remaining").and_then(|v| v.as_f64()) {
                let used = (100.0 - remaining).max(0.0).min(100.0);
                lines.push(MetricLine::Progress {
                    label: "Chat".into(),
                    used,
                    limit: 100.0,
                    format: MetricFormat { kind: "percent".into(), suffix: None },
                    resets_at: None,
                });
            }
        }
    }

    // Free tier: limited_user_quotas
    if let (Some(lq), Some(mq)) = (data.get("limited_user_quotas"), data.get("monthly_quotas")) {
        let reset_date = data.get("limited_user_reset_date")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        if let (Some(remaining), Some(total)) = (
            lq.get("chat").and_then(|v| v.as_f64()),
            mq.get("chat").and_then(|v| v.as_f64()),
        ) {
            if total > 0.0 {
                let used = total - remaining;
                let pct = ((used / total) * 100.0).round().min(100.0).max(0.0);
                lines.push(MetricLine::Progress {
                    label: "Chat".into(),
                    used: pct,
                    limit: 100.0,
                    format: MetricFormat { kind: "percent".into(), suffix: None },
                    resets_at: reset_date.clone(),
                });
            }
        }
    }

    if lines.is_empty() {
        lines.push(MetricLine::Badge {
            label: "Status".into(),
            text: "No usage data".into(),
            color: Some("#a3a3a3".into()),
        });
    }

    Ok((plan, lines))
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        None => String::new(),
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
    }
}
