import { useState, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { Layout } from "@/components/Layout";
import { ScriptEditor } from "@/components/ScriptEditor";
import { VoiceSettings } from "@/components/VoiceSettings";
import { AiSettings } from "@/components/AiSettings";
import type { Project } from "@/types";

function App() {
  const [project, setProject] = useState<Project | null>(null);
  const [error, setError] = useState("");
  const [loading, setLoading] = useState(false);
  const [generating, setGenerating] = useState(false);
  const [saving, setSaving] = useState(false);
  const [generatedFiles, setGeneratedFiles] = useState<string[]>([]);
  const [saveStatus, setSaveStatus] = useState("");

  async function pickAndExtract() {
    setError("");
    setGeneratedFiles([]);
    setSaveStatus("");
    const path = await open({
      filters: [
        {
          name: "练习卷",
          extensions: ["pdf", "docx", "doc", "jpg", "jpeg", "png", "webp"],
        },
      ],
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

  async function saveProject() {
    if (!project) return;
    setSaveStatus("");
    setSaving(true);
    try {
      const savedPath = await invoke<string>("save_project_cmd", { project });
      setSaveStatus(`已保存: ${savedPath}`);
    } catch (e) {
      setError(String(e));
    } finally {
      setSaving(false);
    }
  }

  const handleProjectChange = useCallback((updated: Project) => {
    setProject(updated);
  }, []);

  const busy = loading || generating || saving;

  return (
    <Layout
      topBar={
        <>
          <span className="font-medium">ListenForge</span>
          <button
            className="ml-auto border px-2 py-1 text-sm rounded hover:bg-muted"
            onClick={pickAndExtract}
            disabled={busy}
          >
            {loading ? "提取中…" : "打开练习卷"}
          </button>
          {project && (
            <>
              <button
                className="ml-2 border px-2 py-1 text-sm rounded bg-blue-50 hover:bg-blue-100 disabled:opacity-50"
                onClick={generateAudio}
                disabled={busy}
              >
                {generating ? "生成中…" : "生成音频"}
              </button>
              <button
                className="ml-2 border px-2 py-1 text-sm rounded bg-green-50 hover:bg-green-100 disabled:opacity-50"
                onClick={saveProject}
                disabled={busy}
              >
                {saving ? "保存中…" : "保存项目"}
              </button>
            </>
          )}
        </>
      }
      left={
        <div className="text-sm text-muted-foreground space-y-2">
          <div className="font-medium text-foreground">文件信息</div>
          {project ? (
            <>
              <div className="break-all">{project.source_file}</div>
              <div className="text-xs">
                创建: {new Date(project.created_at).toLocaleString("zh-CN")}
              </div>
              <div className="text-xs">
                Parts: {project.parts.length} &nbsp;|&nbsp; Items:{" "}
                {project.parts.reduce((s, p) => s + p.items.length, 0)}
              </div>
            </>
          ) : (
            <span>未打开文件</span>
          )}
          {error && (
            <div className="mt-2 rounded bg-red-50 p-2 text-xs text-red-600 whitespace-pre-wrap">
              {error}
            </div>
          )}
        </div>
      }
      center={
        error && !project ? (
          <div className="text-sm text-red-600 whitespace-pre-wrap">{error}</div>
        ) : project ? (
          <ScriptEditor project={project} onChange={handleProjectChange} />
        ) : (
          <div className="text-sm text-muted-foreground">点"打开练习卷"开始</div>
        )
      }
      right={
        <div className="space-y-4">
          {project ? (
            <VoiceSettings
              project={project}
              onChange={handleProjectChange}
              generatedFiles={generatedFiles}
              saveStatus={saveStatus}
            />
          ) : (
            <div className="text-sm text-muted-foreground">语音 &amp; 导出</div>
          )}
          <hr className="border-border" />
          <AiSettings />
        </div>
      }
    />
  );
}

export default App;
