mod providers;

use std::time::Duration;

use tauri::{
    Manager,
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
};

use providers::{ProviderMeta, ProviderResult, list_providers, probe_provider};

const BLUR_HIDE_DEBOUNCE_MS: u64 = 180;

#[tauri::command]
fn get_providers() -> Vec<ProviderMeta> {
    list_providers()
}

#[tauri::command]
async fn probe(id: String) -> ProviderResult {
    let id_clone = id.clone();
    // Run blocking provider probe in a thread
    tokio::task::spawn_blocking(move || probe_provider(&id_clone))
        .await
        .unwrap_or_else(|e| ProviderResult {
            id: id.clone(),
            name: id.clone(),
            icon: String::new(),
            brand_color: "#666".into(),
            plan: None,
            lines: vec![],
            error: Some(format!("Internal error: {}", e)),
        })
}

#[tauri::command]
async fn probe_all() -> Vec<ProviderResult> {
    let providers = list_providers();
    let mut results = Vec::new();

    for meta in providers {
        let id = meta.id.clone();
        let result = tokio::task::spawn_blocking(move || probe_provider(&id))
            .await
            .unwrap_or_else(|e| ProviderResult {
                id: meta.id.clone(),
                name: meta.name.clone(),
                icon: meta.icon.clone(),
                brand_color: meta.brand_color.clone(),
                plan: None,
                lines: vec![],
                error: Some(format!("Internal error: {}", e)),
            });
        results.push(result);
    }

    results
}

fn toggle_panel(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        if window.is_visible().unwrap_or(false) {
            let _ = window.hide();
        } else {
            show_panel(app, &window);
        }
    }
}

fn show_panel(app: &tauri::AppHandle, window: &tauri::WebviewWindow) {
    // Position near tray (bottom-right for Windows)
    position_window_near_tray(app, window);
    let _ = window.show();
    let _ = window.unminimize();
    let _ = window.set_focus();
}

fn position_window_near_tray(_app: &tauri::AppHandle, window: &tauri::WebviewWindow) {
    // Get primary monitor info
    if let Ok(Some(monitor)) = window.primary_monitor() {
        let monitor_size = monitor.size();
        let monitor_pos = monitor.position();
        let scale = monitor.scale_factor();

        let win_width = (380.0 * scale) as i32;
        let win_height = (520.0 * scale) as i32;
        let margin = (12.0 * scale) as i32;

        // Position at bottom-right, above taskbar
        // Windows taskbar is typically ~40px
        let taskbar_height = (48.0 * scale) as i32;

        let x = monitor_pos.x + monitor_size.width as i32 - win_width - margin;
        let y = monitor_pos.y + monitor_size.height as i32 - win_height - taskbar_height;

        let _ = window.set_position(tauri::Position::Physical(tauri::PhysicalPosition {
            x,
            y,
        }));
    }
}

fn hide_window_if_still_blurred(window: tauri::WebviewWindow) {
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_millis(BLUR_HIDE_DEBOUNCE_MS)).await;

        let is_visible = window.is_visible().unwrap_or(false);
        let is_focused = window.is_focused().unwrap_or(false);

        if is_visible && !is_focused {
            let _ = window.hide();
        }
    });
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                show_panel(app, &window);
            }
        }))
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            // Build context menu
            let show_i = MenuItem::with_id(app, "show", "Show UsageDock", true, None::<&str>)?;
            let quit_i = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show_i, &quit_i])?;

            // Build tray icon
            let _tray = TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .menu(&menu)
                .tooltip("UsageDock - AI Usage Tracker")
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show" => {
                        if let Some(window) = app.get_webview_window("main") {
                            show_panel(app.app_handle(), &window);
                        }
                    }
                    "quit" => {
                        app.exit(0);
                    }
                    _ => {}
                })
                .on_tray_icon_event(|app, event| match event {
                    TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } => {
                        toggle_panel(app.app_handle());
                    }
                    _ => {}
                })
                .build(app)?;

            // Set up focus loss handler to auto-hide
            let _app_handle = app.handle().clone();
            if let Some(window) = app.get_webview_window("main") {
                let w = window.clone();
                window.on_window_event(move |event| {
                    if let tauri::WindowEvent::Focused(false) = event {
                        hide_window_if_still_blurred(w.clone());
                    }
                });
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![get_providers, probe, probe_all])
        .run(tauri::generate_context!())
        .expect("Error while running UsageDock");
}
