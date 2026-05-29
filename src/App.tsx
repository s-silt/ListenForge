import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { Layout } from "@/components/Layout";
import type { Project } from "@/types";

function App() {
  const [project, setProject] = useState<Project | null>(null);
  const [error, setError] = useState("");
  const [loading, setLoading] = useState(false);
  const [generating, setGenerating] = useState(false);
  const [generatedFiles, setGeneratedFiles] = useState<string[]>([]);

  async function pickAndExtract() {
    setError("");
    setGeneratedFiles([]);
    const path = await open({
      filters: [{ name: "练习卷", extensions: ["pdf", "docx", "doc", "jpg", "jpeg", "png", "webp"] }],
    });
    if (typeof path !== "string") return;
    setLoading(true);
    try {
      setProject(await invoke<Project>("extract_script", { path }));
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }

  async function generateAudio() {
    if (!project) return;
    setError("");
    setGeneratedFiles([]);

    const outputDir = await open({ directory: true });
    if (typeof outputDir !== "string") return;

    setGenerating(true);
    try {
      const files = await invoke<string[]>("generate_audio", {
        project,
        outputDir,
      });
      setGeneratedFiles(files);
    } catch (e) {
      setError(String(e));
    } finally {
      setGenerating(false);
    }
  }

  return (
    <Layout
      topBar={
        <>
          <span className="font-medium">ListenForge</span>
          <button
            className="ml-auto border px-2 py-1 text-sm"
            onClick={pickAndExtract}
            disabled={loading || generating}
          >
            {loading ? "提取中…" : "打开练习卷"}
          </button>
          {project && (
            <button
              className="ml-2 border px-2 py-1 text-sm bg-blue-50 hover:bg-blue-100"
              onClick={generateAudio}
              disabled={loading || generating}
            >
              {generating ? "生成中…" : "生成音频"}
            </button>
          )}
        </>
      }
      left={
        <div className="text-sm text-muted-foreground">
          {project?.source_file ?? "未打开文件"}
        </div>
      }
      center={
        error ? (
          <div className="text-sm text-red-600 whitespace-pre-wrap">{error}</div>
        ) : project ? (
          <div className="space-y-3 text-sm">
            {project.parts.map((p) => (
              <div key={p.id}>
                <div className="font-medium">{p.label}</div>
                {p.zh_instruction && (
                  <div className="text-muted-foreground">{p.zh_instruction}</div>
                )}
                <ul className="ml-4 list-disc">
                  {p.items.map((it) => (
                    <li key={it.id}>
                      {it.number != null ? `${it.number}. ` : ""}
                      {it.text}
                    </li>
                  ))}
                </ul>
              </div>
            ))}
          </div>
        ) : (
          <div className="text-sm text-muted-foreground">点"打开练习卷"开始</div>
        )
      }
      right={
        <div className="text-sm text-muted-foreground space-y-2">
          {generatedFiles.length > 0 ? (
            <>
              <div className="font-medium text-green-700">音频生成完成</div>
              <ul className="space-y-1">
                {generatedFiles.map((f) => (
                  <li key={f} className="break-all">{f}</li>
                ))}
              </ul>
            </>
          ) : (
            <span>语音 & 导出</span>
          )}
        </div>
      }
    />
  );
}

export default App;
