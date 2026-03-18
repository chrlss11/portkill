use serde::Serialize;
use std::collections::HashMap;
use std::process::Command;

#[derive(Debug, Serialize, Clone)]
pub struct PortInfo {
    pub port: u16,
    pub pid: u32,
    pub process_name: String,
}

#[tauri::command]
pub fn list_ports() -> Vec<PortInfo> {
    let netstat_output = match Command::new("netstat")
        .args(["-ano", "-p", "tcp"])
        .output()
    {
        Ok(output) => String::from_utf8_lossy(&output.stdout).to_string(),
        Err(_) => return vec![],
    };

    // Collect unique PIDs first, then batch-resolve process names
    let mut pid_port_pairs: Vec<(u16, u32)> = Vec::new();

    for line in netstat_output.lines() {
        let line = line.trim();
        if !line.contains("LISTENING") {
            continue;
        }

        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 5 {
            continue;
        }

        // Parse local address (e.g., "0.0.0.0:3000" or "[::]:3000")
        let addr = parts[1];
        let port: u16 = match addr.rsplit(':').next().and_then(|p| p.parse().ok()) {
            Some(p) => p,
            None => continue,
        };

        let pid: u32 = match parts[4].parse() {
            Ok(p) if p > 0 => p,
            _ => continue,
        };

        pid_port_pairs.push((port, pid));
    }

    // Get process names for all PIDs in one tasklist call
    let process_names = get_process_names();

    let mut ports: Vec<PortInfo> = pid_port_pairs
        .into_iter()
        .map(|(port, pid)| PortInfo {
            port,
            pid,
            process_name: process_names
                .get(&pid)
                .cloned()
                .unwrap_or_else(|| "Unknown".to_string()),
        })
        .collect();

    // Sort by port number
    ports.sort_by_key(|p| p.port);

    // Deduplicate (same port+pid can appear for 0.0.0.0 and [::])
    ports.dedup_by(|a, b| a.port == b.port && a.pid == b.pid);

    ports
}

fn get_process_names() -> HashMap<u32, String> {
    let mut map = HashMap::new();

    let output = match Command::new("tasklist")
        .args(["/FO", "CSV", "/NH"])
        .output()
    {
        Ok(output) => String::from_utf8_lossy(&output.stdout).to_string(),
        Err(_) => return map,
    };

    for line in output.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        // Format: "name.exe","PID","Session Name","Session#","Mem Usage"
        let fields: Vec<&str> = line.split(',').collect();
        if fields.len() < 2 {
            continue;
        }

        let name = fields[0].trim_matches('"').to_string();
        if let Ok(pid) = fields[1].trim_matches('"').parse::<u32>() {
            map.insert(pid, name);
        }
    }

    map
}

#[tauri::command]
pub fn kill_port(pid: u32) -> Result<String, String> {
    if pid == 0 || pid == 4 {
        return Err("Cannot kill system process".to_string());
    }

    let output = Command::new("taskkill")
        .args(["/PID", &pid.to_string(), "/F"])
        .output()
        .map_err(|e| format!("Failed to execute taskkill: {}", e))?;

    if output.status.success() {
        Ok(format!("Process {} killed", pid))
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!("Failed to kill process {}: {}", pid, stderr.trim()))
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            use tauri::{
                image::Image,
                menu::{MenuBuilder, MenuItemBuilder},
                tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
            };

            let show = MenuItemBuilder::with_id("show", "Abrir PortKill").build(app)?;
            let quit = MenuItemBuilder::with_id("quit", "Salir").build(app)?;
            let menu = MenuBuilder::new(app).items(&[&show, &quit]).build()?;

            let _tray = TrayIconBuilder::new()
                .icon(Image::from_path("icons/icon.png").unwrap_or_else(|_| {
                    Image::from_bytes(include_bytes!("../icons/icon.png"))
                        .expect("Failed to load tray icon")
                }))
                .tooltip("PortKill")
                .menu(&menu)
                .on_menu_event(|app, event| match event.id().as_ref() {
                    "show" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                    "quit" => {
                        app.exit(0);
                    }
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        if let Some(window) = app.get_webview_window("main") {
                            if window.is_visible().unwrap_or(false) {
                                let _ = window.hide();
                            } else {
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                    }
                })
                .build(app)?;

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![list_ports, kill_port])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
