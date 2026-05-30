import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useTranslation } from "react-i18next";
import type { LlmConfigView } from "@/types";

const DEFAULT_BASE_URL = "https://api.openai.com/v1";
const DEFAULT_MODEL = "gpt-5.4-mini";

export function AiSettings() {
  const { t } = useTranslation();
  const [baseUrl, setBaseUrl] = useState(DEFAULT_BASE_URL);
  const [model, setModel] = useState(DEFAULT_MODEL);
  const [newKey, setNewKey] = useState("");
  const [hasApiKey, setHasApiKey] = useState(false);
  const [saving, setSaving] = useState(false);
  const [status, setStatus] = useState("");
  const [statusKind, setStatusKind] = useState<"ok" | "error" | null>(null);

  async function loadConfig() {
    try {
      const cfg = await invoke<LlmConfigView>("get_llm_config");
      setBaseUrl(cfg.base_url);
      setModel(cfg.model);
      setHasApiKey(cfg.has_api_key);
    } catch (e) {
      setStatus(`${t("ai.loadFailed")}: ${String(e)}`);
      setStatusKind("error");
    }
  }

  useEffect(() => {
    loadConfig();
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  async function handleSave() {
    // 基本校验：API 地址须以 http:// 或 https:// 开头
    const url = baseUrl.trim();
    if (!/^https?:\/\//i.test(url)) {
      setStatus(t("ai.invalidUrl"));
      setStatusKind("error");
      return;
    }
    setSaving(true);
    setStatus("");
    setStatusKind(null);
    try {
      await invoke("save_llm_config", {
        baseUrl: url,
        model: model.trim(),
        apiKey: newKey.trim() !== "" ? newKey.trim() : null,
      });
      setNewKey("");
      setStatus(t("ai.savedOk"));
      setStatusKind("ok");
      await loadConfig();
    } catch (e) {
      setStatus(`${t("ai.saveFailed")}: ${String(e)}`);
      setStatusKind("error");
    } finally {
      setSaving(false);
    }
  }

  const keyPlaceholder = hasApiKey ? t("ai.keyConfigured") : t("ai.keyMissing");

  // 用显式 statusKind 判断红/绿，而非反推文案（切换语言也不会错配）
  const isError = statusKind === "error";

  return (
    <div className="space-y-3 text-sm">
      <div>
        <div className="font-medium">{t("ai.title")}</div>
        <div className="text-xs text-muted-foreground mt-0.5">{t("ai.titleHelper")}</div>
      </div>

      {/* First-use warning when no API key configured */}
      {!hasApiKey && (
        <div className="rounded border border-amber-300 bg-amber-50 px-2 py-1.5 text-xs text-amber-700">
          {t("ai.firstUseWarning")}
        </div>
      )}

      {/* API URL */}
      <div className="space-y-1">
        <label className="text-xs text-muted-foreground">{t("ai.apiUrl")}</label>
        <input
          type="text"
          value={baseUrl}
          onChange={(e) => setBaseUrl(e.target.value)}
          placeholder={DEFAULT_BASE_URL}
          className="w-full rounded border border-input bg-background px-2 py-1 text-xs outline-none focus:border-ring"
        />
      </div>

      {/* Model */}
      <div className="space-y-1">
        <label className="text-xs text-muted-foreground">{t("ai.model")}</label>
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
        <label className="text-xs text-muted-foreground">{t("ai.apiKey")}</label>
        <input
          type="password"
          value={newKey}
          onChange={(e) => setNewKey(e.target.value)}
          placeholder={keyPlaceholder}
          autoComplete="new-password"
          className="w-full rounded border border-input bg-background px-2 py-1 text-xs outline-none focus:border-ring"
        />
      </div>

      {/* Save button */}
      <button
        onClick={handleSave}
        disabled={saving}
        className="w-full rounded border border-primary/40 px-2 py-1 text-xs bg-primary/10 text-primary hover:bg-primary/20 disabled:opacity-50 transition-colors"
      >
        {saving ? t("ai.saving") : t("ai.save")}
      </button>

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
    </div>
  );
}
