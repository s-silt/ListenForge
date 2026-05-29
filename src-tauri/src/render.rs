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

// 用 Mutex 保护初始化竞争（OnceLock::get_or_init 是 infallible）
static INIT_LOCK: Mutex<()> = Mutex::new(());

fn get_pdfium() -> Result<&'static Pdfium, String> {
    if let Some(result) = PDFIUM.get() {
        return result.as_ref().map(|h| &h.0).map_err(|e| e.clone());
    }

    // 加锁防止竞争
    let _guard = INIT_LOCK.lock().unwrap();

    // double-check after lock
    if let Some(result) = PDFIUM.get() {
        return result.as_ref().map(|h| &h.0).map_err(|e| e.clone());
    }

    let init_result: Result<PdfiumHolder, String> = {
        Pdfium::bind_to_library(Pdfium::pdfium_platform_library_name_at_path(
            env!("CARGO_MANIFEST_DIR"),
        ))
        .map_err(|e| format!("加载 pdfium 库失败: {e}"))
        .map(|bindings| PdfiumHolder(Pdfium::new(bindings)))
    };

    let _ = PDFIUM.set(init_result);

    PDFIUM
        .get()
        .unwrap()
        .as_ref()
        .map(|h| &h.0)
        .map_err(|e| e.clone())
}

/// 把 PDF 每页渲染成 PNG 字节。scale 提高分辨率以利 OCR（如 2.0）。
pub fn render_pdf_to_pngs(pdf_path: &str, scale: f32) -> Result<Vec<Vec<u8>>, String> {
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

    const TEST_PDF: &str = r"C:\Users\sxl\Documents\ListenForge\Unit 2小练习.pdf";
    const PNG_MAGIC: [u8; 4] = [0x89, 0x50, 0x4E, 0x47];

    #[test]
    #[serial]
    fn render_pdf_produces_valid_pngs() {
        let pages = render_pdf_to_pngs(TEST_PDF, 2.0)
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
