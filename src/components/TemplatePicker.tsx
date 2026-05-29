import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { PromptTemplate } from "@/types";

export function TemplatePicker() {
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
      setStatus(`加载模板失败: ${String(e)}`);
    }
  }

  useEffect(() => {
    loadTemplates();
  }, []);

  function handleSelectChange(id: string) {
    setSelectedId(id);
    const tpl = templates.find((t) => t.id === id);
    if (tpl) setContent(tpl.content);
    setStatus("");
  }

  async function handleApply() {
    setStatus("");
    try {
      await invoke("save_prompt_selection", { id: selectedId });
      setStatus("已应用,下次打开练习卷生效");
    } catch (e) {
      setStatus(`应用失败: ${String(e)}`);
    }
  }

  async function handleSaveAs() {
    const name = window.prompt("新模板名称:", "");
    if (name === null) return; // user cancelled
    if (!name.trim()) {
      setStatus("模板名称不能为空");
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
      setStatus(`已另存为"${name.trim()}"`);
    } catch (e) {
      setStatus(`保存失败: ${String(e)}`);
    } finally {
      setSavingNew(false);
    }
  }

  const currentTpl = templates.find((t) => t.id === selectedId);

  return (
    <div className="space-y-3 text-sm">
      <div className="font-medium">提取模板</div>

      {/* 下拉选择模板 */}
      <div className="space-y-1">
        <label className="text-xs text-muted-foreground">选择模板</label>
        <select
          value={selectedId}
          onChange={(e) => handleSelectChange(e.target.value)}
          className="w-full rounded border border-input bg-background px-2 py-1 text-xs outline-none focus:border-ring"
        >
          {templates.map((t) => (
            <option key={t.id} value={t.id}>
              {t.name}
              {t.builtin ? " (预设)" : ""}
            </option>
          ))}
        </select>
      </div>

      {/* 模板内容编辑区 */}
      <div className="space-y-1">
        <label className="text-xs text-muted-foreground">
          模板内容
          {currentTpl?.builtin && (
            <span className="ml-1 text-amber-600">(内置预设,可编辑后另存)</span>
          )}
        </label>
        <textarea
          value={content}
          onChange={(e) => setContent(e.target.value)}
          rows={8}
          className="w-full rounded border border-input bg-background px-2 py-1 text-xs outline-none focus:border-ring font-mono resize-y"
        />
      </div>

      {/* 操作按钮 */}
      <div className="flex gap-2">
        <button
          onClick={handleApply}
          className="flex-1 rounded border px-2 py-1 text-xs bg-blue-50 hover:bg-blue-100"
        >
          应用此模板
        </button>
        <button
          onClick={handleSaveAs}
          disabled={savingNew}
          className="flex-1 rounded border px-2 py-1 text-xs bg-green-50 hover:bg-green-100 disabled:opacity-50"
        >
          {savingNew ? "保存中…" : "另存为新模板"}
        </button>
      </div>

      {/* 状态提示 */}
      {status && (
        <div
          className={`break-all rounded px-2 py-1 text-xs ${
            status.includes("失败")
              ? "bg-red-50 text-red-600"
              : "bg-green-50 text-green-700"
          }`}
        >
          {status}
        </div>
      )}

      {/* 说明 */}
      <p className="text-xs text-muted-foreground leading-relaxed">
        模板决定"怎么从卷子提取"(标准 / 通用 / 单词 / 中英对照 / 对话)。
        改了或切换模板后点"应用此模板",下次打开练习卷时生效。
      </p>
    </div>
  );
}
