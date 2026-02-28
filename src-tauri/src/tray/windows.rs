//! Windows system tray implementation.

use std::path::PathBuf;
use tauri::{
    image::Image,
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::TrayIconBuilder,
    Manager, WebviewUrl, WebviewWindow,
};
use tauri_plugin_dialog::DialogExt;
use tracing::{error, info, warn};
use windows::Win32::UI::WindowsAndMessaging::{
    SetForegroundWindow, ShowWindow, SW_RESTORE, SW_SHOW,
};

use super::{menu_ids, menu_labels, shutdown_engine};

/// Set up the system tray on Windows.
pub fn setup_tray(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let icon = load_tray_icon(app);

    // Create menu items
    let show_item = MenuItem::with_id(app, menu_ids::SHOW, menu_labels::SHOW, true, None::<&str>)?;
    let settings_item = MenuItem::with_id(
        app,
        menu_ids::SETTINGS,
        menu_labels::SETTINGS,
        true,
        None::<&str>,
    )?;
    let about_item =
        MenuItem::with_id(app, menu_ids::ABOUT, menu_labels::ABOUT, true, None::<&str>)?;
    let exit_item = MenuItem::with_id(app, menu_ids::EXIT, menu_labels::EXIT, true, None::<&str>)?;

    // Build menu -- conditionally include test mode item
    let menu = if flowstt_engine::test_mode::is_test_mode() {
        let run_test_item = MenuItem::with_id(
            app,
            menu_ids::RUN_TEST,
            menu_labels::RUN_TEST,
            true,
            None::<&str>,
        )?;
        Menu::with_items(
            app,
            &[
                &show_item,
                &settings_item,
                &about_item,
                &run_test_item,
                &PredefinedMenuItem::separator(app)?,
                &exit_item,
            ],
        )?
    } else {
        Menu::with_items(
            app,
            &[
                &show_item,
                &settings_item,
                &about_item,
                &PredefinedMenuItem::separator(app)?,
                &exit_item,
            ],
        )?
    };

    // Build tray icon
    let _tray = TrayIconBuilder::new()
        .icon(icon)
        .menu(&menu)
        .tooltip("FlowSTT")
        .on_tray_icon_event(|tray, event| {
            if let tauri::tray::TrayIconEvent::DoubleClick { .. } = event {
                show_main_window(tray.app_handle());
            }
        })
        .on_menu_event(|app, event| {
            handle_menu_event(app, &event);
        })
        .build(app)?;

    Ok(())
}

/// Handle menu item clicks.
fn handle_menu_event(app: &tauri::AppHandle, event: &tauri::menu::MenuEvent) {
    match event.id.as_ref() {
        id if id == menu_ids::SHOW => {
            show_main_window(app);
        }
        id if id == menu_ids::SETTINGS => {
            show_config_window(app);
        }
        id if id == menu_ids::ABOUT => {
            show_about_window(app);
        }
        id if id == menu_ids::RUN_TEST => {
            handle_run_test(app);
        }
        id if id == menu_ids::EXIT => {
            shutdown_engine();
            app.exit(0);
        }
        _ => {}
    }
}

/// Handle the "Run Test (WAV Directory)..." menu item.
/// Opens a native directory picker and starts the test orchestrator.
fn handle_run_test(app: &tauri::AppHandle) {
    if flowstt_engine::test_mode::is_test_run_active() {
        warn!("[TestMode] A test run is already in progress, ignoring request");
        return;
    }

    app.dialog()
        .file()
        .pick_folder(|maybe_dir| match maybe_dir {
            Some(dir) => {
                let path = dir.into_path().expect("Failed to convert dialog path");
                info!("[TestMode] Selected directory: {:?}", path);
                match flowstt_engine::test_mode::start_test_run(path) {
                    Ok(()) => {
                        info!("[TestMode] Test run started");
                    }
                    Err(e) => {
                        error!("[TestMode] Failed to start test run: {}", e);
                    }
                }
            }
            None => {
                info!("[TestMode] Directory picker cancelled");
            }
        });
}

/// Show the main window, recreating if necessary.
fn show_main_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        show_and_focus_window(&window);
    } else {
        // Window was destroyed (can happen on Windows with transparent windows),
        // recreate it
        recreate_main_window(app);
    }
}

/// Show and focus a window using Win32 APIs.
fn show_and_focus_window(window: &WebviewWindow) {
    let _ = window.show();
    let _ = window.unminimize();

    if let Ok(hwnd) = window.hwnd() {
        unsafe {
            let _ = ShowWindow(hwnd, SW_RESTORE);
            let _ = ShowWindow(hwnd, SW_SHOW);
            let _ = SetForegroundWindow(hwnd);
        }
    }

    let _ = window.set_focus();
}

/// Recreate the main window when it has been destroyed.
fn recreate_main_window(app: &tauri::AppHandle) {
    // Create main window with same config as tauri.conf.json
    let window =
        tauri::WebviewWindowBuilder::new(app, "main", WebviewUrl::App("index.html".into()))
            .title("FlowSTT")
            .inner_size(600.0, 300.0)
            .min_inner_size(480.0, 240.0)
            .resizable(true)
            .decorations(false)
            .transparent(false)
            .shadow(true)
            .center()
            .build();

    if let Ok(window) = window {
        show_and_focus_window(&window);
    }
}

/// Show the About window.
fn show_about_window(app: &tauri::AppHandle) {
    // Check if already open
    if let Some(window) = app.get_webview_window("about") {
        let _ = window.set_focus();
        return;
    }

    // Create About window
    let _ = tauri::WebviewWindowBuilder::new(app, "about", WebviewUrl::App("about.html".into()))
        .title("About FlowSTT")
        .inner_size(400.0, 310.0)
        .resizable(false)
        .maximizable(false)
        .minimizable(false)
        .decorations(false)
        .transparent(false)
        .shadow(true)
        .skip_taskbar(true)
        .center()
        .build();
}

/// Show the configuration window.
fn show_config_window(app: &tauri::AppHandle) {
    // Check if already open
    if let Some(window) = app.get_webview_window("config") {
        show_and_focus_window(&window);
        return;
    }

    // Create config window
    let _ = tauri::WebviewWindowBuilder::new(app, "config", WebviewUrl::App("config.html".into()))
        .title("FlowSTT Settings")
        .inner_size(480.0, 460.0)
        .resizable(false)
        .maximizable(false)
        .minimizable(false)
        .decorations(true)
        .transparent(false)
        .shadow(true)
        .skip_taskbar(true)
        .center()
        .build();
}

/// Load a tray icon from multiple possible locations.
fn load_tray_icon_from_paths(
    resource_dir: Option<PathBuf>,
    icon_name: &str,
) -> Option<Image<'static>> {
    let resource_dir_clone = resource_dir.clone();
    let icon_paths = [
        // Production: resource_dir/icons/tray/
        resource_dir.map(|p| p.join(format!("icons/tray/{}", icon_name))),
        // Production: resource_dir/icons/ (for main app icons)
        resource_dir_clone.map(|p| p.join(format!("icons/{}", icon_name))),
        // Development: relative paths (tray subdirectory)
        Some(PathBuf::from(format!("icons/tray/{}", icon_name))),
        Some(PathBuf::from(format!("src-tauri/icons/tray/{}", icon_name))),
        // Development: relative paths (main icons directory)
        Some(PathBuf::from(format!("icons/{}", icon_name))),
        Some(PathBuf::from(format!("src-tauri/icons/{}", icon_name))),
        // Absolute path for development (tray)
        Some(PathBuf::from(format!(
            "{}/icons/tray/{}",
            env!("CARGO_MANIFEST_DIR"),
            icon_name
        ))),
        // Absolute path for development (main icons)
        Some(PathBuf::from(format!(
            "{}/icons/{}",
            env!("CARGO_MANIFEST_DIR"),
            icon_name
        ))),
    ];

    for path in icon_paths.iter().flatten() {
        if path.exists() {
            match Image::from_path(path) {
                Ok(img) => {
                    return Some(img.to_owned());
                }
                Err(e) => {
                    eprintln!("[Tray] Failed to load icon from {:?}: {}", path, e);
                }
            }
        }
    }
    None
}

/// Load the tray icon, with fallback to a generated icon if file not found.
fn load_tray_icon(app: &tauri::App) -> Image<'static> {
    // Try loading 32x32 icon first (best for Windows tray)
    load_tray_icon_from_paths(app.path().resource_dir().ok(), "icon.png")
        .or_else(|| load_tray_icon_from_paths(app.path().resource_dir().ok(), "32x32.png"))
        .unwrap_or_else(|| {
            eprintln!("[Tray] Warning: Could not load tray icon, using fallback");
            create_fallback_icon()
        })
}

/// Create a fallback tray icon (blue circle) when no icon file is found.
fn create_fallback_icon() -> Image<'static> {
    let size = 32u32;
    let mut rgba = vec![0u8; (size * size * 4) as usize];

    // Draw a blue circle
    let center = size as f32 / 2.0 - 0.5;
    let radius = size as f32 / 2.0 - 2.0;

    for y in 0..size {
        for x in 0..size {
            let dx = x as f32 - center;
            let dy = y as f32 - center;
            let dist = (dx * dx + dy * dy).sqrt();

            let idx = ((y * size + x) * 4) as usize;
            if dist < radius {
                rgba[idx] = 0x3b; // R (blue: #3b82f6)
                rgba[idx + 1] = 0x82; // G
                rgba[idx + 2] = 0xf6; // B
                rgba[idx + 3] = 0xff; // A
            }
        }
    }

    Image::new_owned(rgba, size, size)
}
