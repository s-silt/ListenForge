import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { LlmConfigView } from "@/types";

const DEFAULT_BASE_URL = "https://api.openai.com/v1";
const DEFAULT_MODEL = "gpt-5.4-mini";

export function AiSettings() {
  const [baseUrl, setBaseUrl] = useState(DEFAULT_BASE_URL);
  const [model, setModel] = useState(DEFAULT_MODEL);
  const [newKey, setNewKey] = useState("");
  const [hasApiKey, setHasApiKey] = useState(false);
  const [saving, setSaving] = useState(false);
  const [status, setStatus] = useState("");

  async function loadConfig() {
    try {
      const cfg = await invoke<LlmConfigView>("get_llm_config");
      setBaseUrl(cfg.base_url);
      setModel(cfg.model);
      setHasApiKey(cfg.has_api_key);
    } catch (e) {
      setStatus(`加载配置失败: ${String(e)}`);
    }
  }

  useEffect(() => {
    loadConfig();
  }, []);

  async function handleSave() {
    setSaving(true);
    setStatus("");
    try {
      await invoke("save_llm_config", {
        baseUrl,
        model,
        apiKey: newKey.trim() !== "" ? newKey : null,
      });
      setNewKey("");
      setStatus("设置已保存");
      await loadConfig();
    } catch (e) {
      setStatus(`保存失败: ${String(e)}`);
    } finally {
      setSaving(false);
    }
  }

  const keyPlaceholder = hasApiKey ? "已配置(留空不改)" : "未配置,请填入 Key";

  return (
    <div className="space-y-3 text-sm">
      <div className="font-medium">AI 设置</div>

      {/* API 地址 */}
      <div className="space-y-1">
        <label className="text-xs text-muted-foreground">API 地址</label>
        <input
          type="text"
          value={baseUrl}
          onChange={(e) => setBaseUrl(e.target.value)}
          placeholder={DEFAULT_BASE_URL}
          className="w-full rounded border border-input bg-background px-2 py-1 text-xs outline-none focus:border-ring"
        />
      </div>

      {/* 模型 */}
      <div className="space-y-1">
        <label className="text-xs text-muted-foreground">模型</label>
        <input
          type="text"
          value={model}
          onChange={(e) => setModel(e.target.value)}
          placeholder={DEFAULT_MODEL}
          className="w-full rounded border border-input bg-background px-2 py-1 text-xs outline-none focus:border-ring"
        />
      </div>

      {/* API Key */}
      <div className="space-y-1">
        <label className="text-xs text-muted-foreground">API Key</label>
        <input
          type="password"
          value={newKey}
          onChange={(e) => setNewKey(e.target.value)}
          placeholder={keyPlaceholder}
          autoComplete="new-password"
          className="w-full rounded border border-input bg-background px-2 py-1 text-xs outline-none focus:border-ring"
        />
      </div>

      {/* 保存按钮 */}
      <button
        onClick={handleSave}
        disabled={saving}
        className="w-full rounded border px-2 py-1 text-xs bg-blue-50 hover:bg-blue-100 disabled:opacity-50"
      >
        {saving ? "保存中…" : "保存设置"}
      </button>

      {/* 状态提示 */}
      {status && (
        <div
          className={`break-all rounded px-2 py-1 text-xs ${
            status.startsWith("保存失败") || status.startsWith("加载配置失败")
              ? "bg-red-50 text-red-600"
              : "bg-green-50 text-green-700"
          }`}
        >
          {status}
        </div>
      )}
    </div>
  );
}
