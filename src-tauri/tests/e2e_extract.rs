/// 端到端集成测试：真实 PDF → LLM 提取 → Project 构建
///
/// 手动运行：
///   cargo test --test e2e_extract -- --ignored --nocapture
///
/// 需要：
///   - ~/Documents/ListenForge/.env 中有效的 OPENAI_API_KEY
///   - 联网环境
///   - C:\Users\sxl\Documents\ListenForge\Unit 2小练习.pdf 存在
#[tokio::test]
#[ignore]
async fn extract_real_pdf() {
    use listenforge_lib::assembler::build_project;
    use listenforge_lib::input_builder::build_blocks;
    use listenforge_lib::llm::{read_llm_config, LlmProvider};
    use listenforge_lib::llm::openai::OpenAiProvider;
    use listenforge_lib::model::SourceType;

    let pdf_path = r"C:\Users\sxl\Documents\ListenForge\Unit 2小练习.pdf";

    // 1. 构建 content blocks（PDF → PNG → base64 Image blocks）
    let blocks = build_blocks(pdf_path).expect("build_blocks 应成功");
    assert!(!blocks.is_empty(), "PDF 应产生至少 1 个 block");
    println!("blocks count: {}", blocks.len());

    // 2. 读取 LLM 配置
    let (cfg, api_key) = read_llm_config().expect("read_llm_config 应成功（需有效 .env）");
    println!("model: {}, base_url: {}", cfg.model, cfg.base_url);

    // 3. 调用 LLM 提取
    let provider = OpenAiProvider::new(cfg, api_key).expect("构建 OpenAiProvider 应成功");
    let extracted = provider.extract(blocks).await.expect("extract 应成功");
    println!("extracted title: {:?}", extracted.title);
    println!("extracted parts count: {}", extracted.parts.len());

    // 4. 组装为 Project
    let project = build_project(extracted, pdf_path, SourceType::PdfText);
    println!("project id: {}", project.id);
    println!("project title: {}", project.title);
    for (i, p) in project.parts.iter().enumerate() {
        println!("\n  part[{}]: label={:?}", i, p.label);
        println!("    zh_instruction={:?}", p.zh_instruction);
        for it in &p.items {
            println!("    [{:?}] {}", it.number, it.text);
        }
    }

    // 5. 断言
    assert!(!project.parts.is_empty(), "Project.parts 应非空");
}
