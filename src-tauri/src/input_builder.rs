use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};

use crate::llm::ContentBlock;

/// 根据文件扩展名构建 ContentBlock 列表。
/// - PDF  → 每页渲染成 PNG → Image { data_url }
/// - jpg/jpeg/png/webp → 读字节 → Image { data_url }
/// - docx → 抽纯文本 → vec![Text(全文)]
/// - 其它 → Err
pub fn build_blocks(path: &str) -> Result<Vec<ContentBlock>, String> {
    let lower = path.to_lowercase();
    let ext = std::path::Path::new(&lower)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");

    match ext {
        "pdf" => build_blocks_from_pdf(path),
        "jpg" | "jpeg" | "png" | "webp" => build_blocks_from_image(path, ext),
        "docx" => build_blocks_from_docx(path),
        other => Err(format!("不支持的文件扩展名: .{other}")),
    }
}

// ─── PDF ─────────────────────────────────────────────────────────────────────

fn build_blocks_from_pdf(path: &str) -> Result<Vec<ContentBlock>, String> {
    // 提前检查文件存在，避免不必要地初始化 pdfium（pdfium 是进程级单例）
    if !std::path::Path::new(path).exists() {
        return Err(format!("文件不存在: {path}"));
    }
    let pages = crate::render::render_pdf_to_pngs(path, 2.0)?;
    let blocks = pages
        .into_iter()
        .map(|bytes| {
            let b64 = BASE64.encode(&bytes);
            ContentBlock::Image {
                data_url: format!("data:image/png;base64,{b64}"),
            }
        })
        .collect();
    Ok(blocks)
}

// ─── 图片 ─────────────────────────────────────────────────────────────────────

fn build_blocks_from_image(path: &str, ext: &str) -> Result<Vec<ContentBlock>, String> {
    let bytes = std::fs::read(path)
        .map_err(|e| format!("读取图片文件失败 ({path}): {e}"))?;
    let mime = match ext {
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "webp" => "image/webp",
        _ => "image/octet-stream",
    };
    let b64 = BASE64.encode(&bytes);
    Ok(vec![ContentBlock::Image {
        data_url: format!("data:{mime};base64,{b64}"),
    }])
}

// ─── DOCX ─────────────────────────────────────────────────────────────────────

/// 用 zip + quick-xml 解析 word/document.xml，提取 <w:t> 文本节点，
/// 用换行拼接成纯文本。
fn build_blocks_from_docx(path: &str) -> Result<Vec<ContentBlock>, String> {
    use quick_xml::events::Event;
    use quick_xml::Reader;
    use std::io::Read as _;

    // 以 ZIP 打开 .docx
    let file = std::fs::File::open(path)
        .map_err(|e| format!("打开 docx 文件失败 ({path}): {e}"))?;
    let mut archive = zip::ZipArchive::new(file)
        .map_err(|e| format!("解析 ZIP(docx) 失败: {e}"))?;

    // 读取 word/document.xml
    let mut xml_bytes = Vec::new();
    {
        let mut entry = archive
            .by_name("word/document.xml")
            .map_err(|e| format!("docx 内未找到 word/document.xml: {e}"))?;
        entry
            .read_to_end(&mut xml_bytes)
            .map_err(|e| format!("读取 word/document.xml 失败: {e}"))?;
    }

    // 解析 XML，提取 <w:t> 文本
    let xml_str =
        String::from_utf8(xml_bytes).map_err(|e| format!("document.xml UTF-8 解码失败: {e}"))?;

    let mut reader = Reader::from_str(&xml_str);
    reader.config_mut().trim_text(false);

    let mut text_parts: Vec<String> = Vec::new();
    let mut inside_wt = false;

    loop {
        match reader.read_event() {
            Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e)) => {
                // <w:t> 或带命名空间前缀的变体
                let name = e.local_name();
                if name.as_ref() == b"t" {
                    inside_wt = true;
                }
            }
            Ok(Event::Text(e)) if inside_wt => {
                let t = e
                    .unescape()
                    .map_err(|e| format!("XML unescape 失败: {e}"))?;
                text_parts.push(t.into_owned());
                inside_wt = false;
            }
            Ok(Event::End(ref e)) => {
                if e.local_name().as_ref() == b"t" {
                    inside_wt = false;
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(format!("XML 解析错误: {e}")),
            _ => {
                inside_wt = false;
            }
        }
    }

    let full_text = text_parts.join("");
    Ok(vec![ContentBlock::Text(full_text)])
}

// ─── 测试 ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    const TEST_PDF: &str = r"C:\Users\sxl\Documents\ListenForge\Unit 2小练习.pdf";

    #[test]
    #[serial]
    fn build_blocks_pdf_produces_image_blocks() {
        let blocks = build_blocks(TEST_PDF).expect("build_blocks PDF 应成功");

        assert!(
            blocks.len() >= 1,
            "PDF 至少应产生 1 个 block，实际 {}",
            blocks.len()
        );

        for (i, block) in blocks.iter().enumerate() {
            match block {
                ContentBlock::Image { data_url } => {
                    assert!(
                        data_url.starts_with("data:image/png;base64,"),
                        "第 {} 个 block 的 data_url 应以 data:image/png;base64, 开头，实际: {}",
                        i + 1,
                        &data_url[..data_url.len().min(60)]
                    );
                }
                ContentBlock::Text(_) => {
                    panic!("PDF 路线不应产生 Text block，第 {} 个 block 却是 Text", i + 1);
                }
            }
        }

        println!(
            "build_blocks PDF: {} 页，首块 data_url 长度 {}",
            blocks.len(),
            match &blocks[0] {
                ContentBlock::Image { data_url } => data_url.len(),
                _ => 0,
            }
        );
    }

    #[test]
    fn build_blocks_unsupported_ext_returns_err() {
        let err = build_blocks("some/file.txt").unwrap_err();
        assert!(err.contains("txt"), "错误信息应提及扩展名: {err}");
    }

    #[test]
    fn build_blocks_missing_pdf_returns_err() {
        let err = build_blocks("nonexistent_file.pdf").unwrap_err();
        // render 或 pdfium 会返回错误
        assert!(!err.is_empty());
    }
}
