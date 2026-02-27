//! macOS system tray implementation.

use std::path::PathBuf;
use tauri::{
    image::Image,
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::TrayIconBuilder,
    Manager, WebviewUrl,
};

use super::{menu_ids, menu_labels, shutdown_engine};

/// Set up the system tray on macOS.
pub fn setup_tray(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let icon = load_tray_icon(app);

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

    let _tray = TrayIconBuilder::new()
        .icon(icon)
        .menu(&menu)
        .tooltip("FlowSTT")
        .on_tray_icon_event(|tray, event| {
            if let tauri::tray::TrayIconEvent::Click { .. } = event {
                show_main_window(tray.app_handle());
            }
        })
        .on_menu_event(|app, event| {
            handle_menu_event(app, &event);
        })
        .build(app)?;

    Ok(())
}

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
            shutdown_engine();
            app.exit(0);
        }
        _ => {}
    }
}

fn show_main_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    } else {
        recreate_main_window(app);
    }
}

fn recreate_main_window(app: &tauri::AppHandle) {
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
        let _ = window.show();
        let _ = window.set_focus();
    }
}

fn show_about_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("about") {
        let _ = window.set_focus();
        return;
    }

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

fn show_config_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("config") {
        let _ = window.show();
        let _ = window.set_focus();
        return;
    }

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

fn load_tray_icon_from_paths(
    resource_dir: Option<PathBuf>,
    icon_name: &str,
) -> Option<Image<'static>> {
    let resource_dir_clone = resource_dir.clone();
    let icon_paths = [
        resource_dir.map(|p| p.join(format!("icons/tray/{}", icon_name))),
        resource_dir_clone.map(|p| p.join(format!("icons/{}", icon_name))),
        Some(PathBuf::from(format!("icons/tray/{}", icon_name))),
        Some(PathBuf::from(format!("src-tauri/icons/tray/{}", icon_name))),
        Some(PathBuf::from(format!("icons/{}", icon_name))),
        Some(PathBuf::from(format!("src-tauri/icons/{}", icon_name))),
        Some(PathBuf::from(format!(
            "{}/icons/tray/{}",
            env!("CARGO_MANIFEST_DIR"),
            icon_name
        ))),
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

fn load_tray_icon(app: &tauri::App) -> Image<'static> {
    load_tray_icon_from_paths(app.path().resource_dir().ok(), "icon.png")
        .or_else(|| load_tray_icon_from_paths(app.path().resource_dir().ok(), "32x32.png"))
        .unwrap_or_else(|| {
            eprintln!("[Tray] Warning: Could not load tray icon, using fallback");
            create_fallback_icon()
        })
}

fn create_fallback_icon() -> Image<'static> {
    let size = 22u32;
    let mut rgba = vec![0u8; (size * size * 4) as usize];

    let center = size as f32 / 2.0 - 0.5;
    let radius = size as f32 / 2.0 - 2.0;

    for y in 0..size {
        for x in 0..size {
            let dx = x as f32 - center;
            let dy = y as f32 - center;
            let dist = (dx * dx + dy * dy).sqrt();

            let idx = ((y * size + x) * 4) as usize;
            if dist < radius {
                rgba[idx] = 0x3b;
                rgba[idx + 1] = 0x82;
                rgba[idx + 2] = 0xf6;
                rgba[idx + 3] = 0xff;
            }
        }
    }

    Image::new_owned(rgba, size, size)
}
