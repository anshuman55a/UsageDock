use super::{MetricLine, MetricFormat};

/// Windsurf stores its state in a VSCode-style SQLite DB
fn get_state_db_paths() -> Vec<String> {
    let data_dir = if cfg!(target_os = "windows") {
        std::env::var("APPDATA").ok()
    } else if cfg!(target_os = "linux") {
        dirs::config_dir().map(|p| p.to_string_lossy().to_string())
    } else {
        dirs::data_dir().map(|p| p.to_string_lossy().to_string())
    };

    match data_dir {
        Some(d) => {
            let base = std::path::PathBuf::from(d);
            vec![
                // Windsurf (stable)
                base.join("Windsurf")
                    .join("User").join("globalStorage").join("state.vscdb")
                    .to_string_lossy().to_string(),
                // Windsurf Next (preview)
                base.join("Windsurf - Next")
                    .join("User").join("globalStorage").join("state.vscdb")
                    .to_string_lossy().to_string(),
            ]
        }
        None => vec![],
    }
}

fn read_db_value(db_path: &str, key: &str) -> Option<String> {
    let conn = rusqlite::Connection::open_with_flags(
        db_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    ).ok()?;

    let sql = format!("SELECT value FROM ItemTable WHERE key = '{}' LIMIT 1", key);
    let mut stmt = conn.prepare(&sql).ok()?;
    stmt.query_row([], |row| row.get(0)).ok()
}

/// Load API key from windsurfAuthStatus
fn load_api_key(db_path: &str) -> Option<String> {
    let value = read_db_value(db_path, "windsurfAuthStatus")?;
    let auth: serde_json::Value = serde_json::from_str(&value).ok()?;
    auth.get("apiKey")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
}

/// Try to find the LS port by scanning running processes
/// Windsurf LS runs with --extension_server_port and --csrf_token flags
fn discover_ls_from_processes() -> Option<(u16, String)> {
    // On Windows, use wmic or tasklist to find the language server process
    #[cfg(target_os = "windows")]
    {
        let output = std::process::Command::new("wmic")
            .args(["process", "where", "name like '%language_server%'", "get", "commandline"])
            .output()
            .ok()?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        parse_ls_args(&stdout)
    }

    #[cfg(target_os = "linux")]
    {
        let output = std::process::Command::new("ps")
            .args(["aux"])
            .output()
            .ok()?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        // Find lines with language_server that have windsurf markers
        for line in stdout.lines() {
            if line.contains("language_server") && line.contains("windsurf") {
                if let Some(result) = parse_ls_args(line) {
                    return Some(result);
                }
            }
        }
        None
    }

    #[cfg(target_os = "macos")]
    {
        None
    }
}

fn parse_ls_args(text: &str) -> Option<(u16, String)> {
    let mut port: Option<u16> = None;
    let mut csrf: Option<String> = None;

    let parts: Vec<&str> = text.split_whitespace().collect();
    for i in 0..parts.len() {
        if parts[i] == "--extension_server_port" || parts[i].starts_with("--extension_server_port=") {
            if parts[i].contains('=') {
                port = parts[i].split('=').nth(1).and_then(|p| p.parse().ok());
            } else if i + 1 < parts.len() {
                port = parts[i + 1].parse().ok();
            }
        }
        if parts[i] == "--csrf_token" || parts[i].starts_with("--csrf_token=") {
            if parts[i].contains('=') {
                csrf = parts[i].split('=').nth(1).map(|s| s.to_string());
            } else if i + 1 < parts.len() {
                csrf = Some(parts[i + 1].to_string());
            }
        }
    }

    match (port, csrf) {
        (Some(p), Some(c)) => Some((p, c)),
        _ => None,
    }
}

/// Call the LS GetUserStatus endpoint
fn call_ls_get_user_status(port: u16, scheme: &str, csrf: &str, api_key: &str, ide_name: &str) -> Result<serde_json::Value, String> {
    let client = reqwest::blocking::Client::builder()
        .danger_accept_invalid_certs(true)
        .build()
        .map_err(|e| format!("HTTP client error: {}", e))?;

    let url = format!("{}://127.0.0.1:{}/exa.language_server_pb.LanguageServerService/GetUserStatus", scheme, port);

    let body = serde_json::json!({
        "metadata": {
            "apiKey": api_key,
            "ideName": ide_name,
            "ideVersion": "unknown",
            "extensionName": ide_name,
            "extensionVersion": "unknown",
            "locale": "en"
        }
    });

    let resp = client
        .post(&url)
        .header("Content-Type", "application/json")
        .header("Connect-Protocol-Version", "1")
        .header("x-codeium-csrf-token", csrf)
        .json(&body)
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .map_err(|e| format!("LS request failed: {}", e))?;

    let status = resp.status().as_u16();
    if status < 200 || status >= 300 {
        return Err(format!("LS returned HTTP {}", status));
    }

    resp.json::<serde_json::Value>()
        .map_err(|e| format!("Invalid LS response: {}", e))
}

/// Try connecting to the LS on a given port with both schemes
fn try_port(port: u16, csrf: &str, api_key: &str, ide_name: &str) -> Option<serde_json::Value> {
    // Try HTTPS first (LS may use self-signed cert), then HTTP
    for scheme in &["https", "http"] {
        match call_ls_get_user_status(port, scheme, csrf, api_key, ide_name) {
            Ok(data) => return Some(data),
            Err(_) => continue,
        }
    }
    None
}

pub fn probe() -> Result<(Option<String>, Vec<MetricLine>), String> {
    let db_paths = get_state_db_paths();

    // Find the first DB that exists and has credentials
    let mut api_key = None;
    let mut variant_name = "windsurf";

    for (i, db_path) in db_paths.iter().enumerate() {
        if std::path::Path::new(db_path).exists() {
            if let Some(key) = load_api_key(db_path) {
                api_key = Some(key);
                variant_name = if i == 0 { "windsurf" } else { "windsurf-next" };
                break;
            }
        }
    }

    let api_key = api_key.ok_or("Windsurf not installed or not signed in.")?;

    // Discover LS process
    let (port, csrf) = discover_ls_from_processes()
        .ok_or("Windsurf language server not running. Start Windsurf and try again.")?;

    // Call GetUserStatus
    let data = try_port(port, &csrf, &api_key, variant_name)
        .ok_or("Could not connect to Windsurf language server.")?;

    let user_status = data.get("userStatus")
        .ok_or("No user status in LS response.")?;

    let plan_status = user_status.get("planStatus").cloned().unwrap_or(serde_json::Value::Null);
    let plan_info = plan_status.get("planInfo").cloned().unwrap_or(serde_json::Value::Null);

    let plan = plan_info.get("planName")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    // Billing cycle for reset info
    let plan_end = plan_status.get("planEnd")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let mut lines = Vec::new();

    // Credits are in hundredths — divide by 100
    // Prompt credits
    let prompt_total = plan_status.get("availablePromptCredits").and_then(|v| v.as_f64());
    let prompt_used = plan_status.get("usedPromptCredits").and_then(|v| v.as_f64()).unwrap_or(0.0);

    if let Some(total) = prompt_total {
        if total > 0.0 {
            lines.push(MetricLine::Progress {
                label: "Prompt credits".into(),
                used: prompt_used / 100.0,
                limit: total / 100.0,
                format: MetricFormat { kind: "count".into(), suffix: Some("credits".into()) },
                resets_at: plan_end.clone(),
            });
        }
    }

    // Flex credits
    let flex_total = plan_status.get("availableFlexCredits").and_then(|v| v.as_f64());
    let flex_used = plan_status.get("usedFlexCredits").and_then(|v| v.as_f64()).unwrap_or(0.0);

    if let Some(total) = flex_total {
        if total > 0.0 {
            lines.push(MetricLine::Progress {
                label: "Flex credits".into(),
                used: flex_used / 100.0,
                limit: total / 100.0,
                format: MetricFormat { kind: "count".into(), suffix: Some("credits".into()) },
                resets_at: None,
            });
        }
    }

    if lines.is_empty() {
        // Unlimited credits or no data
        lines.push(MetricLine::Badge {
            label: "Credits".into(),
            text: "Unlimited".into(),
            color: Some("#22c55e".into()),
        });
    }

    Ok((plan, lines))
}
