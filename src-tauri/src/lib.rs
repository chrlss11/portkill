mod commands;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .setup(|app| {
            use tauri::{
                menu::{MenuBuilder, MenuItemBuilder},
                tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
            };

            let show = MenuItemBuilder::with_id("show", "Abrir PortKill").build(app)?;
            let quit = MenuItemBuilder::with_id("quit", "Salir").build(app)?;
            let menu = MenuBuilder::new(app).items(&[&show, &quit]).build()?;

            let _tray = TrayIconBuilder::new()
                .icon(tauri::include_image!("icons/icon.png"))
                .tooltip("PortKill")
                .menu(&menu)
                .on_menu_event(|app, event| match event.id().as_ref() {
                    "show" => {
                        if let Some(window) = app.get_webview_window("main") {
                            show_window_near_tray(&window);
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
                                show_window_near_tray(&window);
                            }
                        }
                    }
                })
                .build(app)?;

            // Hide window when it loses focus (click outside)
            let window = app.get_webview_window("main").unwrap();
            let w = window.clone();
            window.on_window_event(move |event| {
                if let tauri::WindowEvent::Focused(false) = event {
                    let _ = w.hide();
                }
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_ports,
            commands::kill_port,
            commands::open_terminal
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn show_window_near_tray(window: &tauri::WebviewWindow) {
    // Get the monitor where the cursor is (tray area)
    if let Ok(Some(monitor)) = window.current_monitor() {
        let monitor_pos = monitor.position();
        let monitor_size = monitor.size();
        let win_size = window.outer_size().unwrap_or(tauri::PhysicalSize {
            width: 420,
            height: 580,
        });
        let scale = monitor.scale_factor();

        // Position: bottom-right corner, above the taskbar
        // Taskbar is ~48px on Windows, ~25px on macOS menu bar (but tray is bottom-right on Win)
        let taskbar_height = (48.0 * scale) as i32;
        let margin = (8.0 * scale) as i32;

        let x = monitor_pos.x + monitor_size.width as i32 - win_size.width as i32 - margin;
        let y = monitor_pos.y + monitor_size.height as i32 - win_size.height as i32 - taskbar_height;

        let _ = window.set_position(tauri::PhysicalPosition { x, y });
    }

    let _ = window.show();
    let _ = window.set_focus();
}
