import { useState, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { useTranslation } from "react-i18next";
import { Layout } from "@/components/Layout";
import { ScriptEditor } from "@/components/ScriptEditor";
import { VoiceSettings } from "@/components/VoiceSettings";
import { AiSettings } from "@/components/AiSettings";
import { TemplatePicker } from "@/components/TemplatePicker";
import type { Project } from "@/types";

function App() {
  const { t, i18n } = useTranslation();
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
      setSaveStatus(`${t("topbar.save")}: ${savedPath}`);
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
  const currentLang = i18n.language;

  function toggleLang() {
    i18n.changeLanguage(currentLang === "zh" ? "en" : "zh");
  }

  return (
    <Layout
      topBar={
        <>
          <span className="font-medium">{t("topbar.appName")}</span>
          <button
            className="ml-auto border border-border px-2 py-1 text-sm rounded bg-background hover:bg-muted transition-colors"
            onClick={pickAndExtract}
            disabled={busy}
            title={t("topbar.openHelper")}
          >
            {loading ? t("topbar.opening") : t("topbar.open")}
          </button>
          {project && (
            <>
              <button
                className="ml-2 border border-primary/40 px-2 py-1 text-sm rounded bg-primary/10 text-primary hover:bg-primary/20 disabled:opacity-50 transition-colors"
                onClick={generateAudio}
                disabled={busy}
                title={t("topbar.generateHelper")}
              >
                {generating ? t("topbar.generating") : t("topbar.generate")}
              </button>
              <button
                className="ml-2 border border-border px-2 py-1 text-sm rounded bg-muted hover:bg-secondary disabled:opacity-50 transition-colors"
                onClick={saveProject}
                disabled={busy}
              >
                {saving ? t("topbar.saving") : t("topbar.save")}
              </button>
            </>
          )}
          {/* Language switcher */}
          <button
            className="ml-2 border border-border px-2 py-1 text-xs rounded bg-background hover:bg-muted font-mono transition-colors"
            onClick={toggleLang}
            title={currentLang === "zh" ? "Switch to English" : "切换到中文"}
          >
            {t("topbar.langSwitch")}
          </button>
        </>
      }
      left={
        <div className="text-sm text-muted-foreground space-y-2">
          <div className="font-medium text-foreground">{t("fileInfo.title")}</div>
          {project ? (
            <>
              <div className="break-all">{project.source_file}</div>
              <div className="text-xs">
                {t("fileInfo.created")}: {new Date(project.created_at).toLocaleString(currentLang === "zh" ? "zh-CN" : "en-US")}
              </div>
              <div className="text-xs">
                Parts: {project.parts.length} &nbsp;|&nbsp; Items:{" "}
                {project.parts.reduce((s, p) => s + p.items.length, 0)}
              </div>
            </>
          ) : (
            <span>{t("fileInfo.noFile")}</span>
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
          <div className="flex flex-col items-center justify-center h-full gap-3 text-muted-foreground select-none">
            <div className="text-base font-medium text-foreground">
              {t("onboarding.emptyHint")}
            </div>
            <div className="text-xs">{t("onboarding.emptySub")}</div>
          </div>
        )
      }
      right={
        <div className="space-y-4">
          <VoiceSettings
            project={project}
            onChange={handleProjectChange}
            generatedFiles={generatedFiles}
            saveStatus={saveStatus}
          />
          <hr className="border-border" />
          <AiSettings />
          <hr className="border-border" />
          <TemplatePicker />
        </div>
      }
    />
  );
}

export default App;
