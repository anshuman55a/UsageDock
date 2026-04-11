use super::{MetricFormat, MetricLine};

/// Cursor credential and API paths
fn get_state_db_path() -> Option<String> {
    let data_dir = if cfg!(target_os = "windows") {
        std::env::var("APPDATA").ok()
    } else if cfg!(target_os = "linux") {
        dirs::config_dir().map(|p| p.to_string_lossy().to_string())
    } else {
        // macOS
        dirs::data_dir().map(|p| p.to_string_lossy().to_string())
    };

    data_dir.map(|d| {
        let path = std::path::PathBuf::from(d);
        if cfg!(target_os = "macos") {
            path.join("Cursor")
                .join("User")
                .join("globalStorage")
                .join("state.vscdb")
                .to_string_lossy()
                .to_string()
        } else {
            path.join("Cursor")
                .join("User")
                .join("globalStorage")
                .join("state.vscdb")
                .to_string_lossy()
                .to_string()
        }
    })
}

const BASE_URL: &str = "https://api2.cursor.sh";
const CLIENT_ID: &str = "KbZUR41cY7W6zRSdpSUJ7I7mLYBKOCmB";

fn read_db_value(db_path: &str, key: &str) -> Option<String> {
    let conn = rusqlite::Connection::open_with_flags(
        db_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .ok()?;

    let mut stmt = conn
        .prepare("SELECT value FROM ItemTable WHERE key = ?1 LIMIT 1")
        .ok()?;
    let result: Option<String> = stmt.query_row([key], |row| row.get(0)).ok();
    result
}

fn refresh_token(refresh_token_value: &str) -> Result<String, String> {
    let client = reqwest::blocking::Client::new();
    let resp = client
        .post(format!("{}/oauth/token", BASE_URL))
        .header("Content-Type", "application/json")
        .json(&serde_json::json!({
            "grant_type": "refresh_token",
            "client_id": CLIENT_ID,
            "refresh_token": refresh_token_value
        }))
        .timeout(std::time::Duration::from_secs(15))
        .send()
        .map_err(|e| format!("Network error: {}", e))?;

    let status = resp.status().as_u16();
    if status == 400 || status == 401 {
        return Err("Token expired. Sign in via Cursor app.".into());
    }
    if status < 200 || status >= 300 {
        return Err(format!("Refresh failed (HTTP {})", status));
    }

    let body: serde_json::Value = resp
        .json()
        .map_err(|e| format!("Invalid response: {}", e))?;

    if body.get("shouldLogout").and_then(|v| v.as_bool()) == Some(true) {
        return Err("Session expired. Sign in via Cursor app.".into());
    }

    body.get("access_token")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| "No access token in refresh response".into())
}

fn connect_post(url: &str, token: &str) -> Result<serde_json::Value, String> {
    let client = reqwest::blocking::Client::new();
    let resp = client
        .post(url)
        .header("Authorization", format!("Bearer {}", token))
        .header("Content-Type", "application/json")
        .header("Connect-Protocol-Version", "1")
        .body("{}")
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .map_err(|e| format!("Network error: {}", e))?;

    let status = resp.status().as_u16();
    if status == 401 || status == 403 {
        return Err("Token expired. Sign in via Cursor app.".into());
    }
    if status < 200 || status >= 300 {
        return Err(format!("API error (HTTP {})", status));
    }

    resp.json::<serde_json::Value>()
        .map_err(|e| format!("Invalid JSON: {}", e))
}

fn cents_to_dollars(cents: f64) -> f64 {
    (cents / 100.0 * 100.0).round() / 100.0
}

pub fn probe() -> Result<(Option<String>, Vec<MetricLine>), String> {
    let db_path = get_state_db_path().ok_or("Cannot determine Cursor data path")?;

    if !std::path::Path::new(&db_path).exists() {
        return Err("Cursor not installed or not signed in.".into());
    }

    let access_token = read_db_value(&db_path, "cursorAuth/accessToken");
    let refresh_tok = read_db_value(&db_path, "cursorAuth/refreshToken");

    let token = match (access_token, refresh_tok) {
        (Some(at), _) if !at.is_empty() => at,
        (_, Some(rt)) if !rt.is_empty() => refresh_token(&rt)?,
        _ => return Err("Not logged in. Sign in via Cursor app.".into()),
    };

    // Fetch usage
    let usage_url = format!(
        "{}/aiserver.v1.DashboardService/GetCurrentPeriodUsage",
        BASE_URL
    );
    let usage = connect_post(&usage_url, &token)?;

    // Fetch plan info
    let plan_url = format!("{}/aiserver.v1.DashboardService/GetPlanInfo", BASE_URL);
    let plan_name = connect_post(&plan_url, &token).ok().and_then(|v| {
        v.get("planInfo")
            .and_then(|pi| pi.get("planName"))
            .and_then(|n| n.as_str())
            .map(|s| s.to_string())
    });

    let plan_label = plan_name.as_deref().map(capitalize);

    if usage.get("enabled").and_then(|v| v.as_bool()) != Some(true) {
        if usage.get("planUsage").is_none() {
            return Err("No active Cursor subscription.".into());
        }
    }

    let mut lines = Vec::new();

    // Plan usage
    if let Some(pu) = usage.get("planUsage") {
        let limit = pu.get("limit").and_then(|v| v.as_f64()).unwrap_or(0.0);
        if limit > 0.0 {
            let total_spend = pu.get("totalSpend").and_then(|v| v.as_f64());
            let remaining = pu.get("remaining").and_then(|v| v.as_f64());
            let used = total_spend.unwrap_or_else(|| limit - remaining.unwrap_or(0.0));

            let resets_at = usage
                .get("billingCycleEnd")
                .and_then(|v| v.as_str().or_else(|| v.as_f64().map(|_| "")))
                .and_then(|_| {
                    usage.get("billingCycleEnd").and_then(|v| {
                        let ms = if let Some(s) = v.as_str() {
                            s.parse::<i64>().ok()
                        } else {
                            v.as_i64()
                        };
                        ms.map(|m| {
                            let dt = chrono_lite_ms_to_iso(m);
                            dt
                        })
                    })
                });

            lines.push(MetricLine::Progress {
                label: "Plan usage".into(),
                used: cents_to_dollars(used),
                limit: cents_to_dollars(limit),
                format: MetricFormat {
                    kind: "dollars".into(),
                    suffix: None,
                },
                resets_at,
            });
        }
    }

    // On-demand spend limit
    if let Some(su) = usage.get("spendLimitUsage") {
        let limit = su
            .get("individualLimit")
            .or_else(|| su.get("pooledLimit"))
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);
        let remaining = su
            .get("individualRemaining")
            .or_else(|| su.get("pooledRemaining"))
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);

        if limit > 0.0 {
            let used = limit - remaining;
            lines.push(MetricLine::Progress {
                label: "On-demand".into(),
                used: cents_to_dollars(used),
                limit: cents_to_dollars(limit),
                format: MetricFormat {
                    kind: "dollars".into(),
                    suffix: None,
                },
                resets_at: None,
            });
        }
    }

    if lines.is_empty() {
        lines.push(MetricLine::Badge {
            label: "Status".into(),
            text: "No usage data".into(),
            color: Some("#a3a3a3".into()),
        });
    }

    Ok((plan_label, lines))
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        None => String::new(),
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
    }
}

fn chrono_lite_ms_to_iso(ms: i64) -> String {
    let secs = ms / 1000;
    let nanos = ((ms % 1000) * 1_000_000) as u32;
    // Simple UTC ISO format
    let d = std::time::UNIX_EPOCH + std::time::Duration::new(secs as u64, nanos);
    let datetime: std::time::SystemTime = d;
    let duration = datetime.duration_since(std::time::UNIX_EPOCH).unwrap();
    let total_secs = duration.as_secs();
    let days = total_secs / 86400;
    let time_secs = total_secs % 86400;
    let hours = time_secs / 3600;
    let minutes = (time_secs % 3600) / 60;
    let seconds = time_secs % 60;

    // Simple date calculation from days since epoch
    let (year, month, day) = days_to_ymd(days as i64);
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        year, month, day, hours, minutes, seconds
    )
}

fn days_to_ymd(days: i64) -> (i64, i64, i64) {
    // Algorithm from https://howardhinnant.github.io/date_algorithms.html
    let z = days + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m as i64, d as i64)
}
