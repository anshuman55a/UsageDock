use super::{MetricFormat, MetricLine};

const REFRESH_URL: &str = "https://auth.openai.com/oauth/token";
const USAGE_URL: &str = "https://chatgpt.com/backend-api/wham/usage";
const CLIENT_ID: &str = "app_EMoamEEZ73f0CkXaXp7hrann";

fn get_auth_paths() -> Vec<std::path::PathBuf> {
    let mut paths = Vec::new();

    // Check CODEX_HOME env var first
    if let Ok(codex_home) = std::env::var("CODEX_HOME") {
        let home = codex_home.trim().to_string();
        if !home.is_empty() {
            paths.push(std::path::PathBuf::from(&home).join("auth.json"));
            return paths;
        }
    }

    if let Some(home) = dirs::home_dir() {
        paths.push(home.join(".config").join("codex").join("auth.json"));
        paths.push(home.join(".codex").join("auth.json"));
    }

    paths
}

fn load_auth() -> Result<serde_json::Value, String> {
    let paths = get_auth_paths();

    for path in &paths {
        if path.exists() {
            let content =
                std::fs::read_to_string(path).map_err(|e| format!("Failed to read auth: {}", e))?;
            let auth: serde_json::Value =
                serde_json::from_str(&content).map_err(|e| format!("Invalid auth file: {}", e))?;

            // Check for valid auth data
            if auth
                .get("tokens")
                .and_then(|t| t.get("access_token"))
                .is_some()
            {
                return Ok(auth);
            }
        }
    }

    Err("Not logged in. Run `codex` to authenticate.".into())
}

fn refresh_token(refresh_tok: &str) -> Result<String, String> {
    let client = reqwest::blocking::Client::new();
    let body = format!(
        "grant_type=refresh_token&client_id={}&refresh_token={}",
        urlencoding_encode(CLIENT_ID),
        urlencoding_encode(refresh_tok)
    );

    let resp = client
        .post(REFRESH_URL)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(body)
        .timeout(std::time::Duration::from_secs(15))
        .send()
        .map_err(|e| format!("Network error: {}", e))?;

    let status = resp.status().as_u16();
    if status == 400 || status == 401 {
        return Err("Session expired. Run `codex` to log in again.".into());
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

fn fetch_usage(
    access_token: &str,
    account_id: Option<&str>,
) -> Result<(serde_json::Value, std::collections::HashMap<String, String>), String> {
    let client = reqwest::blocking::Client::new();
    let mut req = client
        .get(USAGE_URL)
        .header("Authorization", format!("Bearer {}", access_token))
        .header("Accept", "application/json")
        .header("User-Agent", "DevMeter");

    if let Some(aid) = account_id {
        req = req.header("ChatGPT-Account-Id", aid);
    }

    let resp = req
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .map_err(|e| format!("Network error: {}", e))?;

    let status = resp.status().as_u16();
    if status == 401 || status == 403 {
        return Err("Token expired. Run `codex` to log in again.".into());
    }
    if status < 200 || status >= 300 {
        return Err(format!("Usage request failed (HTTP {})", status));
    }

    // Collect relevant headers
    let mut headers = std::collections::HashMap::new();
    for key in [
        "x-codex-primary-used-percent",
        "x-codex-secondary-used-percent",
        "x-codex-credits-balance",
    ] {
        if let Some(val) = resp.headers().get(key) {
            if let Ok(s) = val.to_str() {
                headers.insert(key.to_string(), s.to_string());
            }
        }
    }

    let body: serde_json::Value = resp
        .json()
        .map_err(|e| format!("Invalid response: {}", e))?;

    Ok((body, headers))
}

fn extract_reset_at(value: Option<&serde_json::Value>) -> Option<String> {
    value.and_then(|v| {
        v.as_i64()
            .map(|n| n.to_string())
            .or_else(|| v.as_u64().map(|n| n.to_string()))
            .or_else(|| v.as_str().map(|s| s.to_string()))
    })
}

pub fn probe() -> Result<(Option<String>, Vec<MetricLine>), String> {
    let auth = load_auth()?;

    let tokens = auth.get("tokens").ok_or("No tokens in auth file")?;

    let access_token = tokens
        .get("access_token")
        .and_then(|v| v.as_str())
        .ok_or("No access token")?;

    let account_id = tokens.get("account_id").and_then(|v| v.as_str());

    // Try refresh if needed
    let token = if let Some(refresh_tok) = tokens.get("refresh_token").and_then(|v| v.as_str()) {
        // Check if last refresh was more than 8 days ago
        let needs_refresh = auth
            .get("last_refresh")
            .and_then(|v| v.as_str())
            .map(|_| false) // simplified: don't force refresh
            .unwrap_or(true);

        if needs_refresh {
            match refresh_token(refresh_tok) {
                Ok(new_token) => new_token,
                Err(_) => access_token.to_string(),
            }
        } else {
            access_token.to_string()
        }
    } else {
        access_token.to_string()
    };

    let (data, headers) = fetch_usage(&token, account_id)?;

    let mut lines = Vec::new();

    // Session usage (from headers or body)
    let session_pct = headers
        .get("x-codex-primary-used-percent")
        .and_then(|v| v.parse::<f64>().ok())
        .or_else(|| {
            data.get("rate_limit")
                .and_then(|rl| rl.get("primary_window"))
                .and_then(|pw| pw.get("used_percent"))
                .and_then(|v| v.as_f64())
        });

    if let Some(pct) = session_pct {
        let resets_at = extract_reset_at(
            data.get("rate_limit")
                .and_then(|rl| rl.get("primary_window"))
                .and_then(|pw| pw.get("reset_at")),
        );
        lines.push(MetricLine::Progress {
            label: "Session".into(),
            used: pct,
            limit: 100.0,
            format: MetricFormat {
                kind: "percent".into(),
                suffix: None,
            },
            resets_at,
        });
    }

    // Weekly usage
    let weekly_pct = headers
        .get("x-codex-secondary-used-percent")
        .and_then(|v| v.parse::<f64>().ok())
        .or_else(|| {
            data.get("rate_limit")
                .and_then(|rl| rl.get("secondary_window"))
                .and_then(|sw| sw.get("used_percent"))
                .and_then(|v| v.as_f64())
        });

    if let Some(pct) = weekly_pct {
        let resets_at = extract_reset_at(
            data.get("rate_limit")
                .and_then(|rl| rl.get("secondary_window"))
                .and_then(|sw| sw.get("reset_at")),
        );
        lines.push(MetricLine::Progress {
            label: "Weekly".into(),
            used: pct,
            limit: 100.0,
            format: MetricFormat {
                kind: "percent".into(),
                suffix: None,
            },
            resets_at,
        });
    }

    // Credits balance
    let credits = headers
        .get("x-codex-credits-balance")
        .and_then(|v| v.parse::<f64>().ok())
        .or_else(|| {
            data.get("credits")
                .and_then(|c| c.get("balance"))
                .and_then(|v| v.as_f64())
        });

    if let Some(remaining) = credits {
        let limit = 1000.0;
        let used = (limit - remaining).max(0.0).min(limit);
        lines.push(MetricLine::Progress {
            label: "Credits".into(),
            used,
            limit,
            format: MetricFormat {
                kind: "count".into(),
                suffix: Some("credits".into()),
            },
            resets_at: None,
        });
    }

    let plan = data
        .get("plan_type")
        .and_then(|v| v.as_str())
        .map(capitalize);

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

fn urlencoding_encode(s: &str) -> String {
    let mut result = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                result.push(b as char);
            }
            _ => {
                result.push_str(&format!("%{:02X}", b));
            }
        }
    }
    result
}
