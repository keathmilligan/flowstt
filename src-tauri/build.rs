fn main() {
    // Ensure the release DLL directory satisfies the Tauri resource glob on Windows.
    //
    // `tauri.windows.conf.json` declares `"../target/release/*.dll": "binaries/"` for
    // bundling whisper.cpp libraries. `tauri_build::build()` validates that this glob
    // resolves to at least one file and aborts if it doesn't.
    //
    // The whisper DLLs are downloaded and placed there by the `flowstt-engine` build
    // script, which also runs during debug builds specifically for this purpose. However,
    // Cargo may execute *this* build script before the engine's build script when both
    // crates need to be built from scratch (e.g. after `cargo clean`). In that case the
    // glob validation fails before the DLLs exist.
    //
    // Work around the race by creating a temporary placeholder DLL when the directory is
    // empty. The engine build script overwrites it with the real library once it finishes.
    #[cfg(target_os = "windows")]
    ensure_release_dll_placeholder();

    tauri_build::build();
}

/// Create a placeholder file in `target/release/` so the Tauri resource glob
/// `../target/release/*.dll` matches at least one entry.
#[cfg(target_os = "windows")]
fn ensure_release_dll_placeholder() {
    use std::path::PathBuf;

    let out_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap_or_default());
    let target_dir = out_dir
        .ancestors()
        .find(|p| p.file_name().map(|n| n == "target").unwrap_or(false))
        .map(|p| p.to_path_buf());

    let Some(target_dir) = target_dir else {
        return;
    };

    let release_dir = target_dir.join("release");
    let _ = std::fs::create_dir_all(&release_dir);

    // If there are already real DLLs, nothing to do.
    if let Ok(entries) = std::fs::read_dir(&release_dir) {
        for entry in entries.flatten() {
            if entry
                .path()
                .extension()
                .map(|e| e == "dll")
                .unwrap_or(false)
            {
                return;
            }
        }
    }

    // Create a zero-byte placeholder; the engine build script will replace it.
    let placeholder = release_dir.join(".tauri-placeholder.dll");
    let _ = std::fs::File::create(&placeholder);
}
