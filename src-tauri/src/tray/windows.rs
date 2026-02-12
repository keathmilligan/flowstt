//! Windows system tray implementation.

use std::path::PathBuf;
use tauri::{
    image::Image,
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::TrayIconBuilder,
    Manager, WebviewUrl, WebviewWindow,
};
use windows::Win32::UI::WindowsAndMessaging::{
    SetForegroundWindow, ShowWindow, SW_RESTORE, SW_SHOW,
};

use super::{menu_ids, menu_labels};

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

    // Build menu
    let menu = Menu::with_items(
        app,
        &[
            &show_item,
            &settings_item,
            &about_item,
            &PredefinedMenuItem::separator(app)?,
            &exit_item,
        ],
    )?;

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
        id if id == menu_ids::EXIT => {
            app.exit(0);
        }
        _ => {}
    }
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
            .inner_size(900.0, 340.0)
            .resizable(false)
            .decorations(false)
            .transparent(true)
            .shadow(false)
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
        .inner_size(400.0, 280.0)
        .resizable(false)
        .maximizable(false)
        .minimizable(false)
        .decorations(false)
        .transparent(true)
        .shadow(false)
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
        .inner_size(400.0, 320.0)
        .resizable(false)
        .maximizable(false)
        .minimizable(false)
        .decorations(false)
        .transparent(true)
        .shadow(false)
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
