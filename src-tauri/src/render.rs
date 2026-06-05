use image::ImageFormat;
use pdfium_render::prelude::*;
use std::io::Cursor;
use std::sync::{Mutex, OnceLock};

// pdfium 是进程级单例（内部用 OnceCell 存 bindings）。
// 用 OnceLock<Result<...>> 缓存初始化结果，确保全进程只调用一次 Pdfium::new。
//
// Safety: Pdfium 只含 Option<PdfiumLibraryConfig>，
// 实际 FFI 函数指针在 pdfium-render 内部的进程级 OnceCell 中，pdfium C 库本身线程安全。
struct PdfiumHolder(Pdfium);
unsafe impl Send for PdfiumHolder {}
unsafe impl Sync for PdfiumHolder {}

static PDFIUM: OnceLock<Result<PdfiumHolder, String>> = OnceLock::new();

/// 运行时定位 pdfium.dll 所在目录。
/// 依次尝试：
///   1. 当前可执行文件旁（安装目录 / cargo run 的 target/{profile}/）
///   2. CARGO_MANIFEST_DIR（cargo test / dev fallback，编译期常量）
/// 返回第一个包含 pdfium.dll 的目录路径。
fn find_pdfium_dir() -> std::path::PathBuf {
    // 候选 1：exe 旁
    if let Some(exe_dir) = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
    {
        let dll_path = exe_dir.join("pdfium.dll");
        if dll_path.exists() {
            return exe_dir;
        }
    }

    // 候选 2：CARGO_MANIFEST_DIR（编译期常量，cargo test / dev 时有效）
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn get_pdfium() -> Result<&'static Pdfium, String> {
    // OnceLock::get_or_init 保证闭包全进程只执行一次且线程安全，
    // 无需手动 Mutex + double-check + set/unwrap。旧实现的
    // INIT_LOCK.lock().unwrap() 有锁中毒 panic 风险，set 后 get().unwrap()
    // 又依赖隐式不变量——get_or_init 一并消除这两点。
    let result = PDFIUM.get_or_init(|| {
        let pdfium_dir = find_pdfium_dir();
        Pdfium::bind_to_library(Pdfium::pdfium_platform_library_name_at_path(&pdfium_dir))
            .map_err(|e| format!("加载 pdfium 库失败（目录: {}）: {e}", pdfium_dir.display()))
            .map(|bindings| PdfiumHolder(Pdfium::new(bindings)))
    });
    result.as_ref().map(|h| &h.0).map_err(|e| e.clone())
}

/// 串行化所有渲染:pdfium C 库非线程安全,而 PdfiumHolder 是单例 unsafe-Sync 共享,
/// INIT_LOCK 只覆盖初始化、不覆盖渲染本身,故此处再加全局渲染锁,
/// 保证同一时刻只有一个渲染在进行(否则并发 command 会数据竞争/崩溃)。
static RENDER_LOCK: Mutex<()> = Mutex::new(());

/// 抽取 PDF 全部文本（文字型 PDF 有文本层；扫描版会返回空或极少字符）。
/// 使用 RENDER_LOCK 与渲染共享，保证 pdfium 非线程安全访问串行化。
pub fn extract_pdf_text(pdf_path: &str) -> Result<String, String> {
    let _guard = RENDER_LOCK.lock().map_err(|e| format!("渲染锁中毒: {e}"))?;
    let pdfium = get_pdfium()?;
    let document = pdfium
        .load_pdf_from_file(pdf_path, None)
        .map_err(|e| format!("打开 PDF 失败: {e}"))?;
    let mut out = String::new();
    for page in document.pages().iter() {
        let text = page
            .text()
            .map_err(|e| format!("读取页面文本失败: {e}"))?;
        out.push_str(&text.all());
        out.push_str("\n\n");
    }
    Ok(out)
}

/// 把 PDF 每页渲染成 PNG 字节。scale 提高分辨率以利 OCR（如 2.0）。
pub fn render_pdf_to_pngs(pdf_path: &str, scale: f32) -> Result<Vec<Vec<u8>>, String> {
    let _render_guard = RENDER_LOCK.lock().map_err(|e| format!("渲染锁中毒: {e}"))?;
    let pdfium = get_pdfium()?;

    let document = pdfium
        .load_pdf_from_file(pdf_path, None)
        .map_err(|e| format!("打开 PDF 失败: {e}"))?;

    let render_config = PdfRenderConfig::new().scale_page_by_factor(scale);

    let mut pages_png: Vec<Vec<u8>> = Vec::new();

    for page in document.pages().iter() {
        let bitmap = page
            .render_with_config(&render_config)
            .map_err(|e| format!("渲染页面失败: {e}"))?;

        let img = bitmap
            .as_image()
            .map_err(|e| format!("bitmap 转 image 失败: {e}"))?;

        let mut buf = Cursor::new(Vec::new());
        img.write_to(&mut buf, ImageFormat::Png)
            .map_err(|e| format!("编码 PNG 失败: {e}"))?;

        pages_png.push(buf.into_inner());
    }

    Ok(pages_png)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    // 默认指向开发机本地夹具；可用环境变量 LISTENFORGE_TEST_PDF 覆盖。
    const TEST_PDF: &str = r"C:\Users\sxl\Documents\ListenForge\Unit 2小练习.pdf";
    const PNG_MAGIC: [u8; 4] = [0x89, 0x50, 0x4E, 0x47];

    /// 夹具存在则返回路径，否则打印跳过提示并返回 None（使 cargo test 默认全绿）。
    /// 路径优先取环境变量 LISTENFORGE_TEST_PDF，否则用默认 const。
    fn pdf_fixture_or_skip() -> Option<String> {
        let p = std::env::var("LISTENFORGE_TEST_PDF").unwrap_or_else(|_| TEST_PDF.to_string());
        if std::path::Path::new(&p).exists() {
            Some(p)
        } else {
            eprintln!("跳过 PDF 夹具测试：未找到 {p}（设 LISTENFORGE_TEST_PDF 指向真实 PDF 后可运行）");
            None
        }
    }

    #[test]
    #[serial]
    fn extract_pdf_text_returns_text() {
        let Some(pdf) = pdf_fixture_or_skip() else { return };
        let text = extract_pdf_text(&pdf).expect("extract_pdf_text 应成功");
        let char_count = text.trim().chars().count();
        assert!(
            char_count >= 50,
            "文字型 PDF 应提取到至少 50 个字符，实际 {}",
            char_count
        );
        let preview: String = text.chars().take(2000).collect();
        println!("=== extract_pdf_text 前 2000 字 ===\n{preview}\n=== 共 {char_count} 字符 ===");
    }

    #[test]
    #[serial]
    fn render_pdf_produces_valid_pngs() {
        let Some(pdf) = pdf_fixture_or_skip() else { return };
        let pages = render_pdf_to_pngs(&pdf, 2.0)
            .expect("render_pdf_to_pngs 应成功");

        assert!(
            pages.len() >= 1,
            "应至少渲染出 1 页，实际得到 {} 页",
            pages.len()
        );

        for (i, png_bytes) in pages.iter().enumerate() {
            assert!(
                png_bytes.len() >= 4,
                "第 {} 页 PNG 字节太短: {}",
                i + 1,
                png_bytes.len()
            );
            assert_eq!(
                &png_bytes[..4],
                &PNG_MAGIC,
                "第 {} 页不是有效 PNG 魔数，实际为 {:?}",
                i + 1,
                &png_bytes[..4]
            );
        }

        println!("渲染完成: {} 页，首页大小 {} bytes", pages.len(), pages[0].len());
    }
}
