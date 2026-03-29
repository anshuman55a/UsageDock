use super::{MetricFormat, MetricLine};

#[cfg(target_os = "windows")]
use std::collections::BTreeSet;

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x08000000;

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
                    .join("User")
                    .join("globalStorage")
                    .join("state.vscdb")
                    .to_string_lossy()
                    .to_string(),
                // Windsurf Next (preview)
                base.join("Windsurf - Next")
                    .join("User")
                    .join("globalStorage")
                    .join("state.vscdb")
                    .to_string_lossy()
                    .to_string(),
            ]
        }
        None => vec![],
    }
}

fn read_db_value(db_path: &str, key: &str) -> Option<String> {
    let conn = rusqlite::Connection::open_with_flags(
        db_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .ok()?;

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

struct LocalLsDiscovery {
    ports: Vec<u16>,
    csrf: String,
    version: String,
}

#[cfg(target_os = "linux")]
fn get_ps_executable_path() -> std::path::PathBuf {
    for candidate in ["/usr/bin/ps", "/bin/ps"] {
        let path = std::path::PathBuf::from(candidate);
        if path.exists() {
            return path;
        }
    }

    std::path::PathBuf::from("ps")
}

#[cfg(target_os = "windows")]
fn get_powershell_executable_path() -> std::path::PathBuf {
    let mut candidates = Vec::new();

    for env_key in ["WINDIR", "SystemRoot"] {
        if let Some(root) = std::env::var_os(env_key) {
            let base = std::path::PathBuf::from(root);
            candidates.push(
                base.join("System32")
                    .join("WindowsPowerShell")
                    .join("v1.0")
                    .join("powershell.exe"),
            );
            candidates.push(
                base.join("Sysnative")
                    .join("WindowsPowerShell")
                    .join("v1.0")
                    .join("powershell.exe"),
            );
        }
    }

    for candidate in candidates {
        if candidate.exists() {
            return candidate;
        }
    }

    std::path::PathBuf::from("powershell.exe")
}

fn extract_flag(command: &str, flag: &str) -> Option<String> {
    let parts: Vec<&str> = command.split_whitespace().collect();
    let flag_eq = format!("{}=", flag);

    for (i, part) in parts.iter().enumerate() {
        if *part == flag {
            if i + 1 < parts.len() {
                return Some(parts[i + 1].to_string());
            }
        } else if part.starts_with(&flag_eq) {
            return Some(part[flag_eq.len()..].to_string());
        }
    }

    None
}

/// Try to find the LS port by scanning running processes
/// Windsurf LS runs with --extension_server_port and --csrf_token flags
fn discover_ls(variant_marker: &str) -> Option<LocalLsDiscovery> {
    #[cfg(target_os = "windows")]
    {
        discover_windows_ls(variant_marker)
    }

    #[cfg(target_os = "linux")]
    {
        let output = match std::process::Command::new(get_ps_executable_path())
            .args(["aux"])
            .output()
        {
            Ok(output) => output,
            Err(_) => return None,
        };

        let stdout = String::from_utf8_lossy(&output.stdout);
        collect_ls_candidates(&stdout, variant_marker)
            .into_iter()
            .next()
            .map(|(ports, csrf)| LocalLsDiscovery {
                ports,
                csrf,
                version: "unknown".into(),
            })
    }

    #[cfg(target_os = "macos")]
    {
        let _ = variant_marker;
        None
    }
}

#[cfg(target_os = "linux")]
fn collect_ls_candidates(text: &str, variant_marker: &str) -> Vec<(Vec<u16>, String)> {
    let mut candidates = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || !trimmed.contains("language_server") {
            continue;
        }
        let ide_name = extract_flag(trimmed, "--ide_name")
            .unwrap_or_default()
            .to_lowercase();
        if ide_name != variant_marker {
            continue;
        }
        if let Some((extension_port, csrf)) = parse_ls_args(trimmed) {
            let mut ports = Vec::new();
            ports.push(extension_port);
            if !candidates.iter().any(|(existing_ports, existing_csrf)| {
                existing_ports == &ports && existing_csrf == &csrf
            }) {
                candidates.push((ports, csrf));
            }
        }
    }
    candidates
}

fn parse_ls_args(text: &str) -> Option<(u16, String)> {
    let mut extension_port: Option<u16> = None;
    let mut csrf: Option<String> = None;

    let parts: Vec<&str> = text.split_whitespace().collect();
    for i in 0..parts.len() {
        if parts[i] == "--extension_server_port" || parts[i].starts_with("--extension_server_port=")
        {
            if parts[i].contains('=') {
                extension_port = parts[i].split('=').nth(1).and_then(|p| p.parse().ok());
            } else if i + 1 < parts.len() {
                extension_port = parts[i + 1].parse().ok();
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

    match (extension_port, csrf) {
        (Some(p), Some(c)) => Some((p, c)),
        _ => None,
    }
}

#[cfg(target_os = "windows")]
fn run_hidden_powershell(script: &str) -> Option<String> {
    let output = std::process::Command::new(get_powershell_executable_path())
        .creation_flags(CREATE_NO_WINDOW)
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            script,
        ])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

#[cfg(target_os = "windows")]
fn discover_windows_ls(variant_marker: &str) -> Option<LocalLsDiscovery> {
    let process_json = run_hidden_powershell(
        "& { $procs = @(Get-CimInstance Win32_Process | Where-Object { $_.CommandLine -like '*language_server*' } | Select-Object ProcessId, CommandLine); if ($procs.Count -eq 0) { '[]' } else { $procs | ConvertTo-Json -Compress } }",
    )?;

    let processes = parse_windows_json_items(&process_json);

    for item in processes {
        let process_id = item
            .get("ProcessId")
            .and_then(|value| value.as_u64())
            .map(|value| value as u32);
        let command = item
            .get("CommandLine")
            .and_then(|value| value.as_str())
            .map(|value| value.to_string());

        let (process_id, command) = match (process_id, command) {
            (Some(process_id), Some(command)) => (process_id, command),
            _ => continue,
        };

        let ide_name = extract_flag(&command, "--ide_name")
            .unwrap_or_default()
            .to_lowercase();
        if ide_name != variant_marker {
            continue;
        }

        let (extension_port, csrf) = match parse_ls_args(&command) {
            Some(values) => values,
            None => continue,
        };

        let version =
            extract_flag(&command, "--windsurf_version").unwrap_or_else(|| "unknown".into());

        let port_script = format!(
            "& {{ $ports = @(Get-NetTCPConnection -OwningProcess {} -State Listen -ErrorAction SilentlyContinue | Select-Object -ExpandProperty LocalPort); if ($ports.Count -eq 0) {{ '[]' }} else {{ $ports | ConvertTo-Json -Compress }} }}",
            process_id
        );
        let ports_json = run_hidden_powershell(&port_script).unwrap_or_else(|| "[]".into());
        let mut ports = parse_windows_ports(&ports_json);
        if !ports.contains(&extension_port) {
            ports.push(extension_port);
        }

        return Some(LocalLsDiscovery {
            ports,
            csrf,
            version,
        });
    }

    None
}

#[cfg(target_os = "windows")]
fn parse_windows_json_items(raw: &str) -> Vec<serde_json::Value> {
    let value = match serde_json::from_str::<serde_json::Value>(raw) {
        Ok(value) => value,
        Err(_) => return Vec::new(),
    };

    match value {
        serde_json::Value::Array(items) => items,
        serde_json::Value::Null => Vec::new(),
        other => vec![other],
    }
}

#[cfg(target_os = "windows")]
fn parse_windows_ports(raw: &str) -> Vec<u16> {
    let mut ports = BTreeSet::new();
    for item in parse_windows_json_items(raw) {
        if let Some(port) = item.as_u64().and_then(|value| u16::try_from(value).ok()) {
            ports.insert(port);
        }
    }
    ports.into_iter().collect()
}

fn probe_ls_port(
    client: &reqwest::blocking::Client,
    port: u16,
    csrf: &str,
    ide_name: &str,
    version: &str,
) -> bool {
    let url = format!(
        "https://127.0.0.1:{}/exa.language_server_pb.LanguageServerService/GetUnleashData",
        port
    );

    let body = serde_json::json!({
        "context": {
            "properties": {
                "devMode": "false",
                "extensionVersion": version,
                "ide": ide_name,
                "ideVersion": version,
                "os": if cfg!(target_os = "windows") { "windows" } else { "linux" },
            }
        }
    });

    client
        .post(&url)
        .header("Content-Type", "application/json")
        .header("Connect-Protocol-Version", "1")
        .header("x-codeium-csrf-token", csrf)
        .json(&body)
        .timeout(std::time::Duration::from_secs(5))
        .send()
        .is_ok()
}

fn find_working_ls_endpoint(
    client: &reqwest::blocking::Client,
    discovery: &LocalLsDiscovery,
    ide_name: &str,
) -> Option<u16> {
    for &port in &discovery.ports {
        if probe_ls_port(client, port, &discovery.csrf, ide_name, &discovery.version) {
            return Some(port);
        }
    }
    None
}

/// Call the LS GetUserStatus endpoint
fn call_ls_get_user_status(
    client: &reqwest::blocking::Client,
    port: u16,
    csrf: &str,
    api_key: &str,
    ide_name: &str,
    version: &str,
) -> Result<serde_json::Value, String> {
    let url = format!(
        "https://127.0.0.1:{}/exa.language_server_pb.LanguageServerService/GetUserStatus",
        port
    );

    let body = serde_json::json!({
        "metadata": {
            "apiKey": api_key,
            "ideName": ide_name,
            "ideVersion": version,
            "extensionName": ide_name,
            "extensionVersion": version,
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

fn build_local_tls_client() -> Result<reqwest::blocking::Client, String> {
    reqwest::blocking::Client::builder()
        .build()
        .map_err(|e| format!("HTTP client error: {}", e))
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
    let discovery = discover_ls(variant_name)
        .ok_or("Windsurf language server not running. Start Windsurf and try again.")?;

    let client = build_local_tls_client()?;

    let port = find_working_ls_endpoint(&client, &discovery, variant_name)
        .ok_or("Could not verify a trusted HTTPS connection to the Windsurf language server.")?;

    let data = call_ls_get_user_status(
        &client,
        port,
        &discovery.csrf,
        &api_key,
        variant_name,
        &discovery.version,
    )?;

    let user_status = data
        .get("userStatus")
        .ok_or("No user status in LS response.")?;

    let plan_status = user_status
        .get("planStatus")
        .cloned()
        .unwrap_or(serde_json::Value::Null);
    let plan_info = plan_status
        .get("planInfo")
        .cloned()
        .unwrap_or(serde_json::Value::Null);

    let plan = plan_info
        .get("planName")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    // Billing cycle for reset info
    let plan_end = plan_status
        .get("planEnd")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let mut lines = Vec::new();

    // Credits are in hundredths — divide by 100
    // Prompt credits
    let prompt_total = plan_status
        .get("availablePromptCredits")
        .and_then(|v| v.as_f64());
    let prompt_used = plan_status
        .get("usedPromptCredits")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);

    if let Some(total) = prompt_total {
        if total > 0.0 {
            lines.push(MetricLine::Progress {
                label: "Prompt credits".into(),
                used: prompt_used / 100.0,
                limit: total / 100.0,
                format: MetricFormat {
                    kind: "count".into(),
                    suffix: Some("credits".into()),
                },
                resets_at: plan_end.clone(),
            });
        }
    }

    // Flex credits
    let flex_total = plan_status
        .get("availableFlexCredits")
        .and_then(|v| v.as_f64());
    let flex_used = plan_status
        .get("usedFlexCredits")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);

    if let Some(total) = flex_total {
        if total > 0.0 {
            lines.push(MetricLine::Progress {
                label: "Flex credits".into(),
                used: flex_used / 100.0,
                limit: total / 100.0,
                format: MetricFormat {
                    kind: "count".into(),
                    suffix: Some("credits".into()),
                },
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
