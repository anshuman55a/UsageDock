use super::{MetricFormat, MetricLine};

const CRED_FILE: &str = ".claude/.credentials.json";
const USAGE_URL: &str = "https://api.anthropic.com/api/oauth/usage";
const REFRESH_URL: &str = "https://platform.claude.com/v1/oauth/token";
const CLIENT_ID: &str = "9d1c250a-e61b-44d9-88ed-5944d1962f5e";

fn get_credentials_path() -> Option<String> {
    dirs::home_dir().map(|h| h.join(CRED_FILE).to_string_lossy().to_string())
}

fn load_credentials() -> Result<serde_json::Value, String> {
    let path = get_credentials_path().ok_or("Cannot determine home directory")?;

    if !std::path::Path::new(&path).exists() {
        return Err("Not logged in. Run `claude` to authenticate.".into());
    }

    let content =
        std::fs::read_to_string(&path).map_err(|e| format!("Failed to read credentials: {}", e))?;

    let data: serde_json::Value =
        serde_json::from_str(&content).map_err(|e| format!("Invalid credentials file: {}", e))?;

    Ok(data)
}

fn refresh_token(refresh_tok: &str) -> Result<String, String> {
    let client = reqwest::blocking::Client::new();
    let resp = client
        .post(REFRESH_URL)
        .header("Content-Type", "application/json")
        .json(&serde_json::json!({
            "grant_type": "refresh_token",
            "refresh_token": refresh_tok,
            "client_id": CLIENT_ID,
            "scope": "user:profile user:inference user:sessions:claude_code user:mcp_servers"
        }))
        .timeout(std::time::Duration::from_secs(15))
        .send()
        .map_err(|e| format!("Network error: {}", e))?;

    let status = resp.status().as_u16();
    if status == 400 || status == 401 {
        return Err("Session expired. Run `claude` to log in again.".into());
    }
    if status < 200 || status >= 300 {
        return Err(format!("Token refresh failed (HTTP {})", status));
    }

    let body: serde_json::Value = resp
        .json()
        .map_err(|e| format!("Invalid response: {}", e))?;
    body.get("access_token")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| "No access token in refresh response".into())
}

fn fetch_usage(access_token: &str) -> Result<serde_json::Value, String> {
    let client = reqwest::blocking::Client::new();
    let resp = client
        .get(USAGE_URL)
        .header("Authorization", format!("Bearer {}", access_token.trim()))
        .header("Accept", "application/json")
        .header("Content-Type", "application/json")
        .header("anthropic-beta", "oauth-2025-04-20")
        .header("User-Agent", "DevMeter")
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .map_err(|e| format!("Network error: {}", e))?;

    let status = resp.status().as_u16();
    if status == 401 || status == 403 {
        return Err("Token expired. Run `claude` to log in again.".into());
    }
    if status < 200 || status >= 300 {
        return Err(format!("Usage request failed (HTTP {})", status));
    }

    resp.json::<serde_json::Value>()
        .map_err(|e| format!("Invalid response: {}", e))
}

pub fn probe() -> Result<(Option<String>, Vec<MetricLine>), String> {
    let creds = load_credentials()?;

    let oauth = creds
        .get("claudeAiOauth")
        .ok_or("No OAuth credentials found. Run `claude` to authenticate.")?;

    let access_token = oauth
        .get("accessToken")
        .and_then(|v| v.as_str())
        .ok_or("No access token found")?;

    if access_token.is_empty() {
        return Err("Not logged in. Run `claude` to authenticate.".into());
    }

    // Check if token needs refresh
    let needs_refresh = oauth
        .get("expiresAt")
        .and_then(|v| v.as_i64())
        .map(|exp| {
            let now_ms = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as i64;
            now_ms >= exp - 300_000 // 5 minute buffer
        })
        .unwrap_or(false);

    let token = if needs_refresh {
        if let Some(refresh_tok) = oauth.get("refreshToken").and_then(|v| v.as_str()) {
            match refresh_token(refresh_tok) {
                Ok(new_token) => new_token,
                Err(_) => access_token.to_string(), // try with existing
            }
        } else {
            access_token.to_string()
        }
    } else {
        access_token.to_string()
    };

    let data = fetch_usage(&token)?;

    let plan = oauth
        .get("subscriptionType")
        .and_then(|v| v.as_str())
        .map(capitalize);

    let mut lines = Vec::new();

    // Session (5-hour window)
    if let Some(five_hour) = data.get("five_hour") {
        if let Some(util) = five_hour.get("utilization").and_then(|v| v.as_f64()) {
            let resets_at = five_hour
                .get("resets_at")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            lines.push(MetricLine::Progress {
                label: "Session".into(),
                used: util,
                limit: 100.0,
                format: MetricFormat {
                    kind: "percent".into(),
                    suffix: None,
                },
                resets_at,
            });
        }
    }

    // Weekly (7-day window)
    if let Some(seven_day) = data.get("seven_day") {
        if let Some(util) = seven_day.get("utilization").and_then(|v| v.as_f64()) {
            let resets_at = seven_day
                .get("resets_at")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            lines.push(MetricLine::Progress {
                label: "Weekly".into(),
                used: util,
                limit: 100.0,
                format: MetricFormat {
                    kind: "percent".into(),
                    suffix: None,
                },
                resets_at,
            });
        }
    }

    // Extra usage
    if let Some(extra) = data.get("extra_usage") {
        if extra.get("is_enabled").and_then(|v| v.as_bool()) == Some(true) {
            let used = extra
                .get("used_credits")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0);
            let limit = extra
                .get("monthly_limit")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0);
            if limit > 0.0 {
                lines.push(MetricLine::Progress {
                    label: "Extra usage".into(),
                    used: used / 100.0,
                    limit: limit / 100.0,
                    format: MetricFormat {
                        kind: "dollars".into(),
                        suffix: None,
                    },
                    resets_at: None,
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
