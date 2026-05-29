import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useTranslation } from "react-i18next";
import type { PromptTemplate } from "@/types";

export function TemplatePicker() {
  const { t } = useTranslation();
  const [templates, setTemplates] = useState<PromptTemplate[]>([]);
  const [selectedId, setSelectedId] = useState<string>("");
  const [content, setContent] = useState<string>("");
  const [status, setStatus] = useState<string>("");
  const [savingNew, setSavingNew] = useState(false);

  async function loadTemplates(selectId?: string) {
    try {
      const tpls = await invoke<PromptTemplate[]>("get_prompt_templates");
      setTemplates(tpls);
      if (tpls.length > 0) {
        const target = selectId ?? tpls[0].id;
        const found = tpls.find((t) => t.id === target) ?? tpls[0];
        setSelectedId(found.id);
        setContent(found.content);
      }
    } catch (e) {
      setStatus(`${t("template.loadFailed")}: ${String(e)}`);
    }
  }

  useEffect(() => {
    loadTemplates();
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  function handleSelectChange(id: string) {
    setSelectedId(id);
    const tpl = templates.find((tpl) => tpl.id === id);
    if (tpl) setContent(tpl.content);
    setStatus("");
  }

  async function handleApply() {
    setStatus("");
    try {
      await invoke("save_prompt_selection", { id: selectedId });
      setStatus(t("template.applied"));
    } catch (e) {
      setStatus(`${t("template.applyFailed")}: ${String(e)}`);
    }
  }

  async function handleSaveAs() {
    const name = window.prompt(t("template.newNamePrompt"), "");
    if (name === null) return; // user cancelled
    if (!name.trim()) {
      setStatus(t("template.nameRequired"));
      return;
    }
    setSavingNew(true);
    setStatus("");
    try {
      const newId = await invoke<string>("save_custom_prompt", {
        name: name.trim(),
        content,
      });
      await loadTemplates(newId);
      setStatus(`${t("template.savedAs")} "${name.trim()}"`);
    } catch (e) {
      setStatus(`${t("template.saveFailed")}: ${String(e)}`);
    } finally {
      setSavingNew(false);
    }
  }

  const currentTpl = templates.find((tpl) => tpl.id === selectedId);
  const isError = status.includes(t("template.loadFailed").split(":")[0]) ||
    status.includes(t("template.applyFailed").split(":")[0]) ||
    status.includes(t("template.saveFailed").split(":")[0]);

  return (
    <div className="space-y-3 text-sm">
      <div>
        <div className="font-medium">{t("template.title")}</div>
        <div className="text-xs text-muted-foreground mt-0.5">{t("template.titleHelper")}</div>
      </div>

      {/* Template selector */}
      <div className="space-y-1">
        <label className="text-xs text-muted-foreground">{t("template.selectLabel")}</label>
        <select
          value={selectedId}
          onChange={(e) => handleSelectChange(e.target.value)}
          className="w-full rounded border border-input bg-background px-2 py-1 text-xs outline-none focus:border-ring"
        >
          {templates.map((tpl) => (
            <option key={tpl.id} value={tpl.id}>
              {tpl.name}
              {tpl.builtin ? ` ${t("template.builtinBadge")}` : ""}
            </option>
          ))}
        </select>
      </div>

      {/* Template content editor */}
      <div className="space-y-1">
        <label className="text-xs text-muted-foreground">
          {t("template.contentLabel")}
          {currentTpl?.builtin && (
            <span className="ml-1 text-amber-600">{t("template.builtinNote")}</span>
          )}
        </label>
        <textarea
          value={content}
          onChange={(e) => setContent(e.target.value)}
          rows={8}
          className="w-full rounded border border-input bg-background px-2 py-1 text-xs outline-none focus:border-ring font-mono resize-y"
        />
      </div>

      {/* Action buttons */}
      <div className="flex gap-2">
        <button
          onClick={handleApply}
          className="flex-1 rounded border border-primary/40 px-2 py-1 text-xs bg-primary/10 text-primary hover:bg-primary/20 transition-colors"
        >
          {t("template.apply")}
        </button>
        <button
          onClick={handleSaveAs}
          disabled={savingNew}
          className="flex-1 rounded border border-border px-2 py-1 text-xs bg-muted hover:bg-secondary disabled:opacity-50 transition-colors"
        >
          {savingNew ? t("template.saving") : t("template.saveAs")}
        </button>
      </div>

      {/* Status */}
      {status && (
        <div
          className={`break-all rounded px-2 py-1 text-xs ${
            isError ? "bg-red-50 text-red-600" : "bg-green-50 text-green-700"
          }`}
        >
          {status}
        </div>
      )}

      {/* Description */}
      <p className="text-xs text-muted-foreground leading-relaxed whitespace-pre-line">
        {t("template.description")}
      </p>
    </div>
  );
}
