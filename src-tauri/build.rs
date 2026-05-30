use std::path::PathBuf;

fn main() {
    // 根据编译目标架构选择对应的 pdfium 源 dll。
    // CARGO_CFG_TARGET_ARCH 由 cargo 传入 build script，值为 "aarch64" 或 "x86_64"。
    let target_arch = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));

    let arch_dll_name = match target_arch.as_str() {
        "x86_64" => "pdfium-x64.dll",
        _        => "pdfium-arm64.dll",  // aarch64 及其他均走 ARM64
    };
    let arch_dll_src = manifest_dir.join(arch_dll_name);

    // 触发条件：任一源 dll 变动，或目标架构切换时重新运行 build.rs
    println!("cargo:rerun-if-changed=pdfium-arm64.dll");
    println!("cargo:rerun-if-changed=pdfium-x64.dll");
    // CARGO_CFG_TARGET_ARCH 在 build script 的 env 中以普通变量形式存在；
    // 监听它可确保切换 --target 时 build.rs 重新执行，避免 src-tauri/pdfium.dll 残留错误架构。
    println!("cargo:rerun-if-env-changed=CARGO_CFG_TARGET_ARCH");
    // 编译期内置的自用 Key：变化时重新编译，确保新 Key 烤进二进制。
    for var in [
        "LISTENFORGE_OPENAI_API_KEY",
        "LISTENFORGE_OPENAI_BASE_URL",
        "LISTENFORGE_OPENAI_MODEL",
        "LISTENFORGE_AZURE_TTS_KEY",
        "LISTENFORGE_AZURE_TTS_REGION",
    ] {
        println!("cargo:rerun-if-env-changed={var}");
    }

    if !arch_dll_src.exists() {
        println!(
            "cargo:warning=未找到 {} (target_arch={})，跳过 pdfium.dll 复制",
            arch_dll_src.display(),
            target_arch,
        );
        tauri_build::build();
        return;
    }

    println!(
        "cargo:warning=pdfium 源 dll: {} ({} bytes)",
        arch_dll_src.display(),
        std::fs::metadata(&arch_dll_src).map(|m| m.len()).unwrap_or(0),
    );

    // ① 复制到 src-tauri/pdfium.dll —— tauri bundle.resources 取这个名字打进安装包
    let bundle_dst = manifest_dir.join("pdfium.dll");
    if let Err(e) = std::fs::copy(&arch_dll_src, &bundle_dst) {
        println!("cargo:warning=复制 {} -> {}: {e}", arch_dll_src.display(), bundle_dst.display());
    } else {
        println!("cargo:warning=已更新 src-tauri/pdfium.dll <- {}", arch_dll_name);
    }

    // ② 复制到 target/{profile}/pdfium.dll  —— exe 旁，供 `cargo run` / tauri dev 加载
    //    同时复制到 target/{profile}/deps/pdfium.dll —— cargo test 的测试二进制所在目录
    // OUT_DIR 形如 …/target/debug/build/<pkg>-<hash>/out，向上三层是 target/{profile}/
    if let Ok(out_dir_str) = std::env::var("OUT_DIR") {
        let out_dir = PathBuf::from(out_dir_str);
        if let Some(target_dir) = out_dir.ancestors().nth(3) {
            for subdir in &["", "deps"] {
                let dst_dir = if subdir.is_empty() {
                    target_dir.to_path_buf()
                } else {
                    target_dir.join(subdir)
                };
                // deps/ 可能还不存在（首次构建），跳过而非 panic
                if *subdir == "deps" && !dst_dir.exists() {
                    continue;
                }
                let dst = dst_dir.join("pdfium.dll");
                if let Err(e) = std::fs::copy(&arch_dll_src, &dst) {
                    println!("cargo:warning=复制 pdfium.dll -> {}: {e}", dst.display());
                } else {
                    println!("cargo:warning=已复制 pdfium.dll -> {}", dst.display());
                }
            }
        }
    }

    tauri_build::build()
}
