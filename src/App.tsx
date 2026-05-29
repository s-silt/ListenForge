import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { Layout } from "@/components/Layout";
import type { Project } from "@/types";

function App() {
  const [project, setProject] = useState<Project | null>(null);
  const [error, setError] = useState("");
  const [loading, setLoading] = useState(false);

  async function pickAndExtract() {
    setError("");
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

  return (
    <Layout
      topBar={
        <>
          <span className="font-medium">ListenForge</span>
          <button
            className="ml-auto border px-2 py-1 text-sm"
            onClick={pickAndExtract}
            disabled={loading}
          >
            {loading ? "提取中…" : "打开练习卷"}
          </button>
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
        <div className="text-sm text-muted-foreground">语音 & 导出(待 M3)</div>
      }
    />
  );
}

export default App;
