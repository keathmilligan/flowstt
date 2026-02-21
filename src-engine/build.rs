//! Build script for flowstt-engine
//!
//! Handles downloading/building whisper.cpp library for transcription.

use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

const WHISPER_VERSION: &str = "1.8.2";
const GITHUB_RELEASE_BASE: &str = "https://github.com/ggml-org/whisper.cpp/releases/download";

fn main() {
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();

    // Check if CUDA feature is enabled (set by Cargo when --features cuda is used)
    let cuda_enabled = env::var("CARGO_FEATURE_CUDA").is_ok();

    // macOS: Link ScreenCaptureKit framework for system audio capture
    if target_os == "macos" {
        println!("cargo:rustc-link-lib=framework=ScreenCaptureKit");
        println!("cargo:rustc-link-lib=framework=CoreMedia");
        println!("cargo:rustc-link-lib=framework=AVFoundation");
    }

    // Linux: Build whisper.cpp from source using CMake
    if target_os == "linux" {
        build_whisper_linux(cuda_enabled);
        return;
    }

    let target_arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR not set"));

    // Use a stable cache directory in target/ rather than OUT_DIR (which changes per build)
    // This ensures downloads are cached across rebuilds
    let stable_cache_dir = out_dir
        .ancestors()
        .find(|p| p.file_name().map(|n| n == "target").unwrap_or(false))
        .map(|p| p.join("whisper-cache"))
        .unwrap_or_else(|| out_dir.join("whisper-cache"));

    // Determine which binary to download and which libraries to extract
    // Windows x64 always uses CUDA-enabled binaries (automatic CPU fallback when no GPU)
    let (zip_name, lib_names): (&str, Vec<&str>) = match (target_os.as_str(), target_arch.as_str())
    {
        ("windows", "x86_64") => {
            println!(
                "cargo:warning=Using CUDA-accelerated whisper.cpp binaries (auto CPU fallback)"
            );
            (
                "whisper-cublas-12.4.0-bin-x64.zip",
                vec![
                    "whisper.dll",
                    "ggml.dll",
                    "ggml-base.dll",
                    "ggml-cpu.dll",
                    "ggml-cuda.dll",
                    // CUDA runtime libraries
                    "cublas64_12.dll",
                    "cublasLt64_12.dll",
                    "cudart64_12.dll",
                    // Additional CUDA libraries required for GPU detection
                    "nvrtc64_120_0.dll",
                    "nvrtc-builtins64_124.dll",
                    "nvblas64_12.dll",
                ],
            )
        }
        ("windows", "x86") => (
            "whisper-bin-Win32.zip",
            vec!["whisper.dll", "ggml.dll", "ggml-base.dll", "ggml-cpu.dll"],
        ),
        ("macos", _) => {
            if cuda_enabled {
                println!("cargo:warning=CUDA feature has no effect on macOS - using Metal acceleration via prebuilt framework");
            }
            (
                &format!("whisper-v{}-xcframework.zip", WHISPER_VERSION) as &str,
                vec!["libwhisper.dylib"],
            )
        }
        _ => {
            println!(
                "cargo:warning=Unsupported platform: {}-{}",
                target_os, target_arch
            );
            return;
        }
    };
    let primary_lib = lib_names[0];

    // Use stable cache directory for downloads (persists across rebuilds)
    fs::create_dir_all(&stable_cache_dir).expect("Failed to create cache directory");

    // Include cuda in cache path to separate CUDA and non-CUDA builds
    let cuda_suffix = if cuda_enabled { "-cuda" } else { "" };
    let zip_path = stable_cache_dir.join(format!(
        "whisper-{}-{}{}.zip",
        WHISPER_VERSION, target_arch, cuda_suffix
    ));
    // Extracted libraries also go in stable cache (versioned to handle updates)
    let lib_output_dir = stable_cache_dir.join(format!(
        "whisper-{}-{}{}-lib",
        WHISPER_VERSION, target_arch, cuda_suffix
    ));
    fs::create_dir_all(&lib_output_dir).expect("Failed to create lib output directory");

    let primary_lib_path = lib_output_dir.join(primary_lib);

    // Check if we already have the primary library
    if primary_lib_path.exists() {
        // Library already cached - skip download
    } else {
        // Download if not cached
        if !zip_path.exists() {
            let url = format!("{}/v{}/{}", GITHUB_RELEASE_BASE, WHISPER_VERSION, zip_name);
            println!("cargo:warning=Downloading whisper.cpp binary from: {}", url);
            download_file(&url, &zip_path).expect("Failed to download whisper.cpp binary");
        }

        // Extract all required libraries
        println!("cargo:warning=Extracting whisper.cpp libraries...");
        extract_libraries(
            &zip_path,
            &lib_output_dir,
            &lib_names,
            &target_os,
            &target_arch,
        )
        .expect("Failed to extract whisper.cpp libraries");
    }

    // Set linker flags
    println!(
        "cargo:rustc-link-search=native={}",
        lib_output_dir.display()
    );

    // Copy all libraries to target directory for runtime
    copy_libraries_to_runtime(&lib_output_dir, &lib_names, &out_dir);

    // macOS/Windows: Also copy to release directory for Tauri bundling (even in debug builds)
    // Tauri's build script validates bundle resources exist at configured paths
    if target_os == "macos" || target_os == "windows" {
        copy_libraries_for_tauri_bundle(&lib_output_dir, &lib_names, &out_dir);
    }

    // Also write the primary library path to a file for runtime discovery
    let lib_path_file = out_dir.join("whisper_lib_path.txt");
    fs::write(
        &lib_path_file,
        primary_lib_path.to_string_lossy().as_bytes(),
    )
    .expect("Failed to write library path file");

    // Rerun if build.rs changes
    println!("cargo:rerun-if-changed=build.rs");
}

/// Build whisper.cpp from source on Linux using CMake
fn build_whisper_linux(cuda_enabled: bool) {
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR not set"));
    let target_arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();

    // Check for CMake
    if !check_cmake_available() {
        panic!(
            "\n\nCMake is required to build whisper.cpp on Linux.\n\
            Please install CMake:\n\
            - Ubuntu/Debian: sudo apt install cmake\n\
            - Fedora: sudo dnf install cmake\n\
            - Arch: sudo pacman -S cmake\n\n"
        );
    }

    // Check for CUDA toolkit if cuda feature is enabled
    if cuda_enabled && !check_cuda_available() {
        panic!(
            "\n\nCUDA feature is enabled but CUDA Toolkit is not found.\n\
            Please install NVIDIA CUDA Toolkit:\n\
            - Ubuntu/Debian: sudo apt install nvidia-cuda-toolkit\n\
            - Or download from: https://developer.nvidia.com/cuda-downloads\n\n\
            If you don't need CUDA, build without the cuda feature:\n\
            cargo build --release\n\n"
        );
    }

    // Use stable cache directory in target/ (persists across rebuilds)
    let stable_cache_dir = out_dir
        .ancestors()
        .find(|p| p.file_name().map(|n| n == "target").unwrap_or(false))
        .map(|p| p.join("whisper-cache"))
        .unwrap_or_else(|| out_dir.join("whisper-cache"));
    fs::create_dir_all(&stable_cache_dir).expect("Failed to create cache directory");

    // Include cuda in paths to separate CUDA and non-CUDA builds
    let cuda_suffix = if cuda_enabled { "-cuda" } else { "" };
    let source_tarball = stable_cache_dir.join(format!("whisper-{}.tar.gz", WHISPER_VERSION));
    let source_dir = stable_cache_dir.join(format!("whisper.cpp-{}", WHISPER_VERSION));
    let build_dir =
        stable_cache_dir.join(format!("whisper-{}-build{}", WHISPER_VERSION, cuda_suffix));
    let lib_output_dir = stable_cache_dir.join(format!(
        "whisper-{}-{}{}-lib",
        WHISPER_VERSION, target_arch, cuda_suffix
    ));

    fs::create_dir_all(&lib_output_dir).expect("Failed to create lib output directory");

    let lib_path = lib_output_dir.join("libwhisper.so");

    // Check if we already have the library built
    if lib_path.exists() {
        println!("cargo:warning=Using cached whisper.cpp library");
    } else {
        // Download source tarball if not cached
        if !source_dir.exists() {
            if !source_tarball.exists() {
                let url = format!(
                    "https://github.com/ggml-org/whisper.cpp/archive/refs/tags/v{}.tar.gz",
                    WHISPER_VERSION
                );
                println!("cargo:warning=Downloading whisper.cpp source from: {}", url);
                download_file(&url, &source_tarball)
                    .expect("Failed to download whisper.cpp source");
            }

            // Extract tarball
            println!("cargo:warning=Extracting whisper.cpp source...");
            extract_tarball(&source_tarball, &stable_cache_dir)
                .expect("Failed to extract whisper.cpp source");
        }

        // Create build directory
        fs::create_dir_all(&build_dir).expect("Failed to create build directory");

        // Configure with CMake
        println!("cargo:warning=Configuring whisper.cpp with CMake...");
        let mut cmake_args = vec![
            source_dir.to_string_lossy().to_string(),
            "-DBUILD_SHARED_LIBS=ON".to_string(),
            "-DCMAKE_BUILD_TYPE=Release".to_string(),
            "-DWHISPER_BUILD_EXAMPLES=OFF".to_string(),
            "-DWHISPER_BUILD_TESTS=OFF".to_string(),
            "-DWHISPER_BUILD_SERVER=OFF".to_string(),
        ];

        if cuda_enabled {
            println!("cargo:warning=CUDA feature enabled - configuring with GPU support");
            cmake_args.push("-DGGML_CUDA=ON".to_string());
        }

        let cmake_status = Command::new("cmake")
            .args(&cmake_args)
            .current_dir(&build_dir)
            .status()
            .expect("Failed to run cmake configure");

        if !cmake_status.success() {
            panic!("CMake configuration failed");
        }

        // Build
        println!("cargo:warning=Building whisper.cpp (this may take a few minutes)...");
        let build_status = Command::new("cmake")
            .args(["--build", ".", "--config", "Release", "-j"])
            .current_dir(&build_dir)
            .status()
            .expect("Failed to run cmake build");

        if !build_status.success() {
            panic!("CMake build failed");
        }

        // Find and copy built libraries
        println!("cargo:warning=Copying built libraries...");
        copy_built_libraries(&build_dir, &lib_output_dir).expect("Failed to copy built libraries");
    }

    // Set linker flags
    println!(
        "cargo:rustc-link-search=native={}",
        lib_output_dir.display()
    );

    // Copy libraries to runtime directory
    let lib_names: Vec<&str> = vec![
        "libwhisper.so",
        "libggml.so",
        "libggml-base.so",
        "libggml-cpu.so",
    ];
    copy_libraries_to_runtime(&lib_output_dir, &lib_names, &out_dir);

    // If CUDA is enabled, also copy CUDA-specific libraries
    if cuda_enabled {
        let cuda_libs = ["libggml-cuda.so"];
        for lib in cuda_libs {
            let src = lib_output_dir.join(lib);
            if src.exists() {
                copy_library_to_runtime(&src, lib, &out_dir);
            }
        }
    }

    // Write library path for runtime discovery
    let lib_path_file = out_dir.join("whisper_lib_path.txt");
    fs::write(&lib_path_file, lib_path.to_string_lossy().as_bytes())
        .expect("Failed to write library path file");

    println!("cargo:warning=Linux build: whisper.cpp built from source with CMake");
    if cuda_enabled {
        println!("cargo:warning=CUDA support enabled");
    }

    println!("cargo:rerun-if-changed=build.rs");
}

/// Check if CMake is available
fn check_cmake_available() -> bool {
    Command::new("cmake")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Check if CUDA toolkit is available (nvcc compiler)
fn check_cuda_available() -> bool {
    Command::new("nvcc")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Extract a .tar.gz file
fn extract_tarball(tarball: &Path, dest: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let status = Command::new("tar")
        .args([
            "-xzf",
            &tarball.to_string_lossy(),
            "-C",
            &dest.to_string_lossy(),
        ])
        .status()?;

    if !status.success() {
        return Err("Failed to extract tarball".into());
    }

    Ok(())
}

/// Copy built libraries from CMake build directory to output
fn copy_built_libraries(
    build_dir: &Path,
    output_dir: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    // Look for shared libraries in the build directory
    // CMake may put them in different locations depending on configuration
    let search_paths = [
        build_dir.to_path_buf(),
        build_dir.join("src"),
        build_dir.join("ggml").join("src"),
        build_dir.join("lib"),
    ];

    let lib_patterns = [
        "libwhisper.so",
        "libggml.so",
        "libggml-base.so",
        "libggml-cpu.so",
        "libggml-cuda.so",
    ];

    for pattern in &lib_patterns {
        for search_path in &search_paths {
            // Look for the library (may have version suffix like .so.1.8.2)
            if let Ok(entries) = fs::read_dir(search_path) {
                for entry in entries.flatten() {
                    let name = entry.file_name();
                    let name_str = name.to_string_lossy();

                    // Match libwhisper.so or libwhisper.so.1.8.2 etc.
                    if name_str.starts_with(pattern) || name_str == *pattern {
                        let src = entry.path();

                        // Follow symlinks to get the actual file
                        let real_src = if src.is_symlink() {
                            fs::read_link(&src).unwrap_or(src.clone())
                        } else {
                            src.clone()
                        };

                        // Resolve relative symlink paths
                        let real_src = if real_src.is_relative() {
                            search_path.join(&real_src)
                        } else {
                            real_src
                        };

                        if real_src.exists() && real_src.is_file() {
                            // Copy as the base name (libwhisper.so)
                            let dest = output_dir.join(pattern);
                            fs::copy(&real_src, &dest)?;
                            println!("cargo:warning=Copied {} from {:?}", pattern, real_src);
                            break;
                        }
                    }
                }
            }
        }
    }

    // Verify libwhisper.so was copied
    if !output_dir.join("libwhisper.so").exists() {
        return Err("libwhisper.so not found in build output".into());
    }

    Ok(())
}

/// Copy libraries to the runtime directory
fn copy_libraries_to_runtime(lib_dir: &Path, lib_names: &[&str], out_dir: &Path) {
    let profile = env::var("PROFILE").unwrap_or_else(|_| "debug".to_string());
    let target_dir = out_dir
        .ancestors()
        .find(|p| p.ends_with("target") || p.file_name().map(|n| n == "target").unwrap_or(false))
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| out_dir.join("..").join("..").join(".."));

    let runtime_lib_dir = target_dir.join(&profile);
    if runtime_lib_dir.exists() {
        for lib_name in lib_names {
            let lib_path = lib_dir.join(lib_name);
            if lib_path.exists() {
                copy_library_to_runtime(&lib_path, lib_name, out_dir);
            }
        }
    }
}

/// Copy a single library to the runtime directory
fn copy_library_to_runtime(lib_path: &Path, lib_name: &str, out_dir: &Path) {
    let profile = env::var("PROFILE").unwrap_or_else(|_| "debug".to_string());
    let target_dir = out_dir
        .ancestors()
        .find(|p| p.ends_with("target") || p.file_name().map(|n| n == "target").unwrap_or(false))
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| out_dir.join("..").join("..").join(".."));

    let runtime_lib_dir = target_dir.join(&profile);
    if runtime_lib_dir.exists() {
        let runtime_lib_path = runtime_lib_dir.join(lib_name);
        if lib_path.exists()
            && (!runtime_lib_path.exists()
                || fs::metadata(lib_path).map(|m| m.len()).unwrap_or(0)
                    != fs::metadata(&runtime_lib_path)
                        .map(|m| m.len())
                        .unwrap_or(0))
        {
            if let Err(e) = fs::copy(lib_path, &runtime_lib_path) {
                println!("cargo:warning=Failed to copy {}: {}", lib_name, e);
            } else {
                println!(
                    "cargo:warning=Copied {} to {}",
                    lib_name,
                    runtime_lib_dir.display()
                );
            }
        }
    }
}

/// Copy libraries to release directory for Tauri bundling (macOS/Windows)
/// Tauri's build script validates bundle resources exist, even during debug builds
fn copy_libraries_for_tauri_bundle(lib_dir: &Path, lib_names: &[&str], out_dir: &Path) {
    let target_dir = out_dir
        .ancestors()
        .find(|p| p.file_name().map(|n| n == "target").unwrap_or(false))
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| out_dir.join("..").join("..").join(".."));

    let release_lib_dir = target_dir.join("release");
    if !release_lib_dir.exists() {
        if let Err(e) = fs::create_dir_all(&release_lib_dir) {
            println!(
                "cargo:warning=Failed to create release directory for Tauri bundle: {}",
                e
            );
            return;
        }
    }

    for lib_name in lib_names {
        let lib_path = lib_dir.join(lib_name);
        let release_lib_path = release_lib_dir.join(lib_name);

        if lib_path.exists()
            && (!release_lib_path.exists()
                || fs::metadata(&lib_path).map(|m| m.len()).unwrap_or(0)
                    != fs::metadata(&release_lib_path)
                        .map(|m| m.len())
                        .unwrap_or(0))
        {
            if let Err(e) = fs::copy(&lib_path, &release_lib_path) {
                println!(
                    "cargo:warning=Failed to copy {} for Tauri bundle: {}",
                    lib_name, e
                );
            } else {
                println!(
                    "cargo:warning=Copied {} to release/ for Tauri bundling",
                    lib_name
                );
            }
        }
    }
}

fn download_file(url: &str, dest: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let response = reqwest::blocking::Client::builder()
        .user_agent("flowstt-build")
        .build()?
        .get(url)
        .send()?;

    if !response.status().is_success() {
        return Err(format!("HTTP error: {} for URL: {}", response.status(), url).into());
    }

    let bytes = response.bytes()?;
    let mut file = fs::File::create(dest)?;
    file.write_all(&bytes)?;

    Ok(())
}

fn extract_libraries(
    zip_path: &Path,
    output_dir: &Path,
    lib_names: &[&str],
    target_os: &str,
    _target_arch: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let file = fs::File::open(zip_path)?;
    let mut archive = zip::ZipArchive::new(file)?;

    if target_os == "macos" {
        // xcframework structure: the macOS binary is in macos-arm64_x86_64 (universal binary)
        // The framework contains the binary at whisper.framework/Versions/A/whisper
        let lib_name = lib_names[0]; // macOS only has one library (libwhisper.dylib)

        // First try: look for the macos universal binary framework
        for i in 0..archive.len() {
            let mut file = archive.by_index(i)?;
            let name = file.name().to_string();

            // Look for the framework binary in the macos folder
            // Path: build-apple/whisper.xcframework/macos-arm64_x86_64/whisper.framework/Versions/A/whisper
            if name.contains("macos-arm64_x86_64")
                && name.contains("whisper.framework/Versions/A/whisper")
                && !name.ends_with("/")
            {
                let output_path = output_dir.join(lib_name);
                let mut output_file = fs::File::create(&output_path)?;
                io::copy(&mut file, &mut output_file)?;
                println!(
                    "cargo:warning=Extracted {} from {} (framework binary)",
                    lib_name, name
                );
                return Ok(());
            }
        }

        // Fallback: look for any dylib
        for i in 0..archive.len() {
            let mut file = archive.by_index(i)?;
            let name = file.name().to_string();

            if name.ends_with(".dylib") && !name.contains("ios") {
                let output_path = output_dir.join(lib_name);
                let mut output_file = fs::File::create(&output_path)?;
                io::copy(&mut file, &mut output_file)?;
                println!(
                    "cargo:warning=Extracted {} from {} (fallback dylib)",
                    lib_name, name
                );
                return Ok(());
            }
        }

        // Second fallback: look for any macos whisper binary (not a directory)
        for i in 0..archive.len() {
            let mut file = archive.by_index(i)?;
            let name = file.name().to_string();

            if name.contains("macos") && name.ends_with("/whisper") && file.size() > 0 {
                let output_path = output_dir.join(lib_name);
                let mut output_file = fs::File::create(&output_path)?;
                io::copy(&mut file, &mut output_file)?;
                println!(
                    "cargo:warning=Extracted {} from {} (second fallback)",
                    lib_name, name
                );
                return Ok(());
            }
        }

        Err("Could not find whisper binary in xcframework".into())
    } else {
        // Windows: find all required DLLs in the archive
        let mut found = vec![false; lib_names.len()];

        for i in 0..archive.len() {
            let mut file = archive.by_index(i)?;
            let name = file.name().to_string();

            for (idx, lib_name) in lib_names.iter().enumerate() {
                if !found[idx] && name.ends_with(lib_name) {
                    let output_path = output_dir.join(lib_name);
                    let mut output_file = fs::File::create(&output_path)?;
                    io::copy(&mut file, &mut output_file)?;
                    println!("cargo:warning=Extracted {}", lib_name);
                    found[idx] = true;
                    break;
                }
            }
        }

        // Check that all required libraries were found
        for (idx, lib_name) in lib_names.iter().enumerate() {
            if !found[idx] {
                return Err(format!("Could not find {} in archive", lib_name).into());
            }
        }

        Ok(())
    }
}
