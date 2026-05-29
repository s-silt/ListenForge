use std::path::PathBuf;

fn main() {
    // 把 src-tauri/pdfium.dll 复制到 target/{profile}/ (exe 旁)，
    // 使 `cargo run` / `tauri dev` 和集成测试都能在 exe 旁找到它。
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let dll_src = manifest_dir.join("pdfium.dll");

    if dll_src.exists() {
        // OUT_DIR 形如 …/target/debug/build/<pkg>-<hash>/out
        // 向上三层就是 target/{profile}/
        let out_dir = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR not set"));
        let target_dir = out_dir
            .ancestors()
            .nth(3)
            .expect("OUT_DIR should be deep enough")
            .to_path_buf();

        let dll_dst = target_dir.join("pdfium.dll");
        if let Err(e) = std::fs::copy(&dll_src, &dll_dst) {
            // 复制失败时只警告，不 panic —— 让 CARGO_MANIFEST_DIR fallback 接手
            println!("cargo:warning=复制 pdfium.dll 到 {}: {e}", dll_dst.display());
        } else {
            println!("cargo:warning=已复制 pdfium.dll -> {}", dll_dst.display());
        }
        // 当 pdfium.dll 变动时重新运行 build.rs
        println!("cargo:rerun-if-changed=pdfium.dll");
    } else {
        println!(
            "cargo:warning=未找到 src-tauri/pdfium.dll，跳过复制（路径: {}）",
            dll_src.display()
        );
    }

    tauri_build::build()
}
