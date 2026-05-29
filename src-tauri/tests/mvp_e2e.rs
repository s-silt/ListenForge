//! 完整 MVP 闭环:真实练习卷 PDF → 提取 → 合成 → 写出 mp3 文件。
//! 手动跑:cargo test --test mvp_e2e -- --ignored --nocapture
//! 需要:~/Documents/ListenForge/.env 有效 key + 联网 + 测试 PDF。
use listenforge_lib::assembler::build_project;
use listenforge_lib::audio::generate_project_audio;
use listenforge_lib::export::save_audio;
use listenforge_lib::input_builder::build_blocks;
use listenforge_lib::llm::openai::OpenAiProvider;
use listenforge_lib::llm::{read_llm_config, LlmProvider};
use listenforge_lib::model::SourceType;
use listenforge_lib::tts::edge::EdgeTtsProvider;

#[tokio::test]
#[ignore]
async fn full_pipeline_pdf_to_mp3() {
    let pdf = r"C:\Users\sxl\Documents\ListenForge\Unit 2小练习.pdf";
    let out_dir = r"C:\Users\sxl\Documents\ListenForge\output";
    std::fs::create_dir_all(out_dir).expect("建输出目录");

    // 1) 提取
    println!("[1/3] 提取听力稿...");
    let blocks = build_blocks(pdf).expect("build_blocks");
    let (cfg, key) = read_llm_config().expect("read_llm_config");
    let llm = OpenAiProvider::new(cfg, key).expect("OpenAiProvider");
    let extracted = llm.extract(blocks).await.expect("extract");
    let project = build_project(extracted, pdf, SourceType::PdfText);
    println!("    标题: {}", project.title);
    println!("    大题数: {}", project.parts.len());

    // 2) 合成音频
    println!("[2/3] edge-tts 逐段合成(约十几句,需时间)...");
    let tts = EdgeTtsProvider::new();
    let (full, parts) = generate_project_audio(&project, &tts).await.expect("generate_project_audio");
    println!("    完整 mp3: {} bytes; 分段 {} 个", full.len(), parts.len());

    // 3) 写出
    println!("[3/3] 写出文件...");
    let paths = save_audio(&full, &parts, out_dir, &project.title).expect("save_audio");
    for p in &paths {
        println!("    写出: {p}");
    }

    assert!(!full.is_empty(), "完整 mp3 应非空");
    assert!(!parts.is_empty(), "应有分段 mp3");
    println!("\n✅ MVP 闭环完成 —— 去 {out_dir} 听完整 mp3");
}
