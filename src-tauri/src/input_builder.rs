use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};

use crate::llm::ContentBlock;

/// 根据文件扩展名构建 ContentBlock 列表。
/// - PDF（文字型）→ pdfium 抽文本 → vec![Text(全文)]
/// - PDF（扫描版）→ pdfium 文本层为空 → 回退渲染 PNG → 多个 Image { data_url }
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

    // 先尝试文本提取：文字型 PDF 有文本层，字符数足够多则走纯文本路线
    let text = crate::render::extract_pdf_text(path)?;
    if text.trim().chars().count() >= 50 {
        // 文字型 PDF：直接返回 Text block，避免昂贵的图像渲染 + vision API
        return Ok(vec![ContentBlock::Text(text)]);
    }

    // 扫描版 PDF（几乎无文本层）：回退到渲染成 PNG → Image block
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
                match name.as_ref() {
                    b"t" => inside_wt = true,
                    // 换行 / 制表符是 <w:r> 内与 <w:t> 平级的空元素
                    b"br" => text_parts.push("\n".to_string()),
                    b"tab" => text_parts.push("\t".to_string()),
                    // 自闭合空段落 <w:p/> 也要产生一个段落分隔
                    b"p" => text_parts.push("\n".to_string()),
                    _ => {}
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
                match e.local_name().as_ref() {
                    b"t" => inside_wt = false,
                    // 段落结束 → 换行，恢复行/段落结构
                    b"p" => text_parts.push("\n".to_string()),
                    _ => {}
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

    // 默认指向开发机本地夹具；可用环境变量 LISTENFORGE_TEST_PDF 覆盖。
    const TEST_PDF: &str = r"C:\Users\sxl\Documents\ListenForge\Unit 2小练习.pdf";

    /// 夹具存在则返回路径，否则打印跳过提示并返回 None（使 cargo test 默认全绿）。
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
    fn build_blocks_pdf_produces_text_block_for_textual_pdf() {
        let Some(pdf) = pdf_fixture_or_skip() else { return };
        let blocks = build_blocks(&pdf).expect("build_blocks PDF 应成功");

        assert_eq!(
            blocks.len(),
            1,
            "文字型 PDF 应产生恰好 1 个 Text block，实际 {} 个",
            blocks.len()
        );

        match &blocks[0] {
            ContentBlock::Text(text) => {
                assert!(
                    !text.trim().is_empty(),
                    "Text block 不应为空"
                );
                // 确认包含字母（中文字符或英文字母，排除全空白/噪声）
                let has_alphanumeric = text.chars().any(|c| c.is_alphabetic());
                assert!(
                    has_alphanumeric,
                    "Text block 应含字母字符，实际前 200 字: {}",
                    &text.chars().take(200).collect::<String>()
                );
                println!(
                    "build_blocks PDF Text block: {} 字符，前 300 字:\n{}",
                    text.chars().count(),
                    &text.chars().take(300).collect::<String>()
                );
            }
            ContentBlock::Image { .. } => {
                panic!("文字型 PDF 应返回 Text block，但返回了 Image block");
            }
        }
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
