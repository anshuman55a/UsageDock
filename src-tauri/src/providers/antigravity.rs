use super::{MetricLine, MetricFormat};
use base64::Engine;

/// Antigravity credential and API paths
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
        path.join("Antigravity")
            .join("User")
            .join("globalStorage")
            .join("state.vscdb")
            .to_string_lossy()
            .to_string()
    })
}

const CLOUD_CODE_URLS: &[&str] = &[
    "https://daily-cloudcode-pa.googleapis.com",
    "https://cloudcode-pa.googleapis.com",
];
const FETCH_MODELS_PATH: &str = "/v1internal:fetchAvailableModels";
const GOOGLE_OAUTH_URL: &str = "https://oauth2.googleapis.com/token";
const GOOGLE_CLIENT_ID: &str = "1071006060591-tmhssin2h21lcre235vtolojh4g403ep.apps.googleusercontent.com";
const GOOGLE_CLIENT_SECRET: &str = "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";

// Models to skip (internal/placeholder)
const MODEL_BLACKLIST: &[&str] = &[
    "MODEL_CHAT_20706",
    "MODEL_CHAT_23310",
    "MODEL_GOOGLE_GEMINI_2_5_FLASH",
    "MODEL_GOOGLE_GEMINI_2_5_FLASH_THINKING",
    "MODEL_GOOGLE_GEMINI_2_5_FLASH_LITE",
    "MODEL_GOOGLE_GEMINI_2_5_PRO",
    "MODEL_PLACEHOLDER_M19",
    "MODEL_PLACEHOLDER_M9",
];

fn read_db_value(db_path: &str, key: &str) -> Option<String> {
    let conn = rusqlite::Connection::open_with_flags(
        db_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    ).ok()?;

    let sql = format!("SELECT value FROM ItemTable WHERE key = '{}' LIMIT 1", key);
    let mut stmt = conn.prepare(&sql).ok()?;
    stmt.query_row([], |row| row.get(0)).ok()
}

/// Load API key from antigravityAuthStatus
fn load_api_key(db_path: &str) -> Option<String> {
    let value = read_db_value(db_path, "antigravityAuthStatus")?;
    let auth: serde_json::Value = serde_json::from_str(&value).ok()?;
    auth.get("apiKey")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
}

/// Simple protobuf varint reader
fn read_varint(data: &[u8], mut pos: usize) -> Option<(u64, usize)> {
    let mut value: u64 = 0;
    let mut shift = 0u32;
    while pos < data.len() {
        let b = data[pos];
        pos += 1;
        value |= ((b & 0x7f) as u64) << shift;
        if (b & 0x80) == 0 {
            return Some((value, pos));
        }
        shift += 7;
        if shift >= 64 {
            return None;
        }
    }
    None
}

/// Parse protobuf fields (field_number -> (wire_type, data))
fn read_proto_fields(data: &[u8]) -> std::collections::HashMap<u64, Vec<u8>> {
    let mut fields = std::collections::HashMap::new();
    let mut pos = 0;
    while pos < data.len() {
        let (tag, new_pos) = match read_varint(data, pos) {
            Some(v) => v,
            None => break,
        };
        pos = new_pos;
        let field_num = tag >> 3;
        let wire_type = tag & 7;

        match wire_type {
            0 => {
                // Varint
                let (val, new_pos) = match read_varint(data, pos) {
                    Some(v) => v,
                    None => break,
                };
                pos = new_pos;
                fields.insert(field_num, val.to_le_bytes().to_vec());
            }
            2 => {
                // Length-delimited
                let (len, new_pos) = match read_varint(data, pos) {
                    Some(v) => v,
                    None => break,
                };
                pos = new_pos;
                let end = pos + len as usize;
                if end > data.len() {
                    break;
                }
                fields.insert(field_num, data[pos..end].to_vec());
                pos = end;
            }
            _ => break,
        }
    }
    fields
}

/// Load protobuf-encoded tokens from the DB
fn load_proto_tokens(db_path: &str) -> Option<(String, Option<String>)> {
    let value = read_db_value(db_path, "jetskiStateSync.agentManagerInitState")?;
    let raw = base64::engine::general_purpose::STANDARD.decode(value.as_bytes()).ok()?;
    let outer = read_proto_fields(&raw);
    let field6 = outer.get(&6)?;
    let inner = read_proto_fields(field6);
    let access_token = inner.get(&1)
        .and_then(|d| String::from_utf8(d.clone()).ok())
        .filter(|s| !s.is_empty())?;
    let refresh_token = inner.get(&3)
        .and_then(|d| String::from_utf8(d.clone()).ok())
        .filter(|s| !s.is_empty());

    Some((access_token, refresh_token))
}

/// Refresh Google OAuth token
fn refresh_access_token(refresh_token: &str) -> Result<String, String> {
    let client = reqwest::blocking::Client::new();
    let body = format!(
        "client_id={}&client_secret={}&refresh_token={}&grant_type=refresh_token",
        urlencoding_encode(GOOGLE_CLIENT_ID),
        urlencoding_encode(GOOGLE_CLIENT_SECRET),
        urlencoding_encode(refresh_token)
    );

    let resp = client
        .post(GOOGLE_OAUTH_URL)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(body)
        .timeout(std::time::Duration::from_secs(15))
        .send()
        .map_err(|e| format!("Network error: {}", e))?;

    let status = resp.status().as_u16();
    if status < 200 || status >= 300 {
        return Err(format!("Google OAuth refresh failed (HTTP {})", status));
    }

    let body: serde_json::Value = resp.json().map_err(|e| format!("Invalid response: {}", e))?;
    body.get("access_token")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| "No access token in refresh response".into())
}

/// Fetch models from Cloud Code API using the token
fn fetch_cloud_code(token: &str) -> Result<serde_json::Value, String> {
    let client = reqwest::blocking::Client::new();

    for base_url in CLOUD_CODE_URLS {
        let url = format!("{}{}", base_url, FETCH_MODELS_PATH);
        match client
            .post(&url)
            .header("Content-Type", "application/json")
            .header("Authorization", format!("Bearer {}", token))
            .header("User-Agent", "antigravity")
            .body("{}")
            .timeout(std::time::Duration::from_secs(15))
            .send()
        {
            Ok(resp) => {
                let status = resp.status().as_u16();
                if status == 401 || status == 403 {
                    return Err("auth_failed".into());
                }
                if status >= 200 && status < 300 {
                    return resp.json::<serde_json::Value>()
                        .map_err(|e| format!("Invalid response: {}", e));
                }
            }
            Err(e) => {
                log::warn!("Cloud Code request to {} failed: {}", base_url, e);
            }
        }
    }

    Err("All Cloud Code endpoints failed".into())
}

/// Parse model configs from Cloud Code response
fn parse_models(data: &serde_json::Value) -> Vec<(String, f64, Option<String>)> {
    let models = match data.get("models").and_then(|v| v.as_object()) {
        Some(m) => m,
        None => return vec![],
    };

    let mut results: std::collections::HashMap<String, (f64, Option<String>)> = std::collections::HashMap::new();

    for (key, model) in models {
        if model.get("isInternal").and_then(|v| v.as_bool()) == Some(true) {
            continue;
        }

        let model_id = model.get("model")
            .and_then(|v| v.as_str())
            .unwrap_or(key.as_str());

        if MODEL_BLACKLIST.contains(&model_id) {
            continue;
        }

        let display_name = match model.get("displayName").and_then(|v| v.as_str()) {
            Some(n) if !n.trim().is_empty() => normalize_label(n.trim()),
            _ => continue,
        };

        let quota_info = match model.get("quotaInfo") {
            Some(qi) => qi,
            None => continue,
        };

        let remaining = match quota_info.get("remainingFraction").and_then(|v| v.as_f64()) {
            Some(f) => f,
            None => continue,
        };

        let reset_time = quota_info.get("resetTime")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        // Deduplicate by display name, keep the lower remaining
        let entry = results.entry(display_name.clone()).or_insert((remaining, reset_time.clone()));
        if remaining < entry.0 {
            *entry = (remaining, reset_time);
        }
    }

    let mut sorted: Vec<(String, f64, Option<String>)> = results
        .into_iter()
        .map(|(name, (remaining, reset))| (name, remaining, reset))
        .collect();

    sorted.sort_by(|a, b| model_sort_key(&a.0).cmp(&model_sort_key(&b.0)));
    sorted
}

fn normalize_label(label: &str) -> String {
    // Remove trailing parenthetical like " (High)"
    if let Some(idx) = label.rfind('(') {
        let before = label[..idx].trim();
        if !before.is_empty() {
            return before.to_string();
        }
    }
    label.to_string()
}

fn model_sort_key(label: &str) -> String {
    let lower = label.to_lowercase();
    if lower.contains("gemini") && lower.contains("pro") {
        format!("0a_{}", label)
    } else if lower.contains("gemini") {
        format!("0b_{}", label)
    } else if lower.contains("claude") && lower.contains("opus") {
        format!("1a_{}", label)
    } else if lower.contains("claude") {
        format!("1b_{}", label)
    } else {
        format!("2_{}", label)
    }
}

pub fn probe() -> Result<(Option<String>, Vec<MetricLine>), String> {
    let db_path = get_state_db_path()
        .ok_or("Cannot determine Antigravity data path")?;

    if !std::path::Path::new(&db_path).exists() {
        return Err("Antigravity not installed or not signed in.".into());
    }

    // Collect possible tokens
    let mut tokens: Vec<String> = Vec::new();

    // Try protobuf-encoded tokens first (most common path)
    let refresh_tok = if let Some((access, refresh)) = load_proto_tokens(&db_path) {
        tokens.push(access);
        refresh
    } else {
        None
    };

    // Try API key
    if let Some(api_key) = load_api_key(&db_path) {
        if !tokens.contains(&api_key) {
            tokens.push(api_key);
        }
    }

    if tokens.is_empty() {
        return Err("Start Antigravity and sign in.".into());
    }

    // Try each token against Cloud Code
    let mut cc_data = None;
    for token in &tokens {
        match fetch_cloud_code(token) {
            Ok(data) => {
                cc_data = Some(data);
                break;
            }
            Err(ref e) if e == "auth_failed" => continue,
            Err(_) => continue,
        }
    }

    // If all tokens failed, try refresh
    if cc_data.is_none() {
        if let Some(ref rt) = refresh_tok {
            if let Ok(new_token) = refresh_access_token(rt) {
                if let Ok(data) = fetch_cloud_code(&new_token) {
                    cc_data = Some(data);
                }
            }
        }
    }

    let data = cc_data.ok_or("Start Antigravity and try again.")?;

    let models = parse_models(&data);
    if models.is_empty() {
        return Err("No model usage data available.".into());
    }

    let mut lines = Vec::new();
    for (name, remaining_fraction, reset_time) in models {
        let clamped = remaining_fraction.max(0.0).min(1.0);
        let used = ((1.0 - clamped) * 100.0).round();

        lines.push(MetricLine::Progress {
            label: name,
            used,
            limit: 100.0,
            format: MetricFormat { kind: "percent".into(), suffix: None },
            resets_at: reset_time,
        });
    }

    Ok((None, lines))
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
