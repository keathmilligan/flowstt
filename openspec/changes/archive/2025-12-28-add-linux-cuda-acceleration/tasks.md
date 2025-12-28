## 1. Implementation

- [x] 1.1 Add `[features]` section to `src-tauri/Cargo.toml` with `cuda` feature
- [x] 1.2 Configure `cuda` feature to enable `whisper-rs/cuda` for Linux builds
- [x] 1.3 Test build without CUDA feature (verify no regression)
- [x] 1.4 Test build with CUDA feature on Linux with NVIDIA GPU (requires CUDA Toolkit)

## 2. Documentation

- [x] 2.1 Update README with CUDA build instructions for Linux
- [x] 2.2 Document CUDA Toolkit requirements (nvcc, cuBLAS)
- [x] 2.3 Add troubleshooting section for common CUDA build issues
