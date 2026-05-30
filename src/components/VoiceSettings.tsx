import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useTranslation } from "react-i18next";
import type { Project, VoiceConfig, Voice, AzureConfigView } from "@/types";

interface VoiceSettingsProps {
  project: Project | null;
  onChange: (updated: Project) => void;
  generatedFiles: string[];
  saveStatus: { text: string; ok: boolean } | null;
}

function patch(project: Project, vc: Partial<VoiceConfig>): Project {
  return { ...project, voice_config: { ...project.voice_config, ...vc } };
}

function VoiceSelect({
  label,
  value,
  voices,
  disabled,
  onChange,
}: {
  label: string;
  value: string;
  voices: Voice[];
  disabled: boolean;
  onChange: (v: string) => void;
}) {
  return (
    <div className="space-y-1">
      <label className="text-xs text-muted-foreground">{label}</label>
      <select
        value={value}
        disabled={disabled}
        onChange={(e) => onChange(e.target.value)}
        className="w-full rounded border border-input bg-background px-2 py-1 text-xs outline-none focus:border-ring disabled:opacity-50"
      >
        {voices.map((v) => (
          <option key={v.id} value={v.id}>
            {v.label}
          </option>
        ))}
        {/* keep current value if not in list */}
        {!voices.some((v) => v.id === value) && value && (
          <option value={value}>{value}</option>
        )}
      </select>
    </div>
  );
}

export function VoiceSettings({
  project,
  onChange,
  generatedFiles,
  saveStatus,
}: VoiceSettingsProps) {
  const { t } = useTranslation();
  const [voices, setVoices] = useState<Voice[]>([]);
  const [azureRegion, setAzureRegion] = useState("");
  const [azureKey, setAzureKey] = useState("");
  const [azureHasKey, setAzureHasKey] = useState(false);
  const [azureSaving, setAzureSaving] = useState(false);
  const [azureStatus, setAzureStatus] = useState<{ text: string; ok: boolean } | null>(null);

  useEffect(() => {
    invoke<Voice[]>("get_voices")
      .then(setVoices)
      .catch((e) => {
        // 保留当前下拉值；记录错误便于排查（避免在 catch 里静默吞错）
        console.error("get_voices 失败:", e);
      });
  }, []);

  useEffect(() => {
    invoke<AzureConfigView>("get_azure_tts_config")
      .then((c) => {
        setAzureRegion(c.region);
        setAzureHasKey(c.has_key);
      })
      .catch((e) => console.error("get_azure_tts_config 失败:", e));
  }, []);

  async function saveAzure() {
    setAzureSaving(true);
    setAzureStatus(null);
    try {
      await invoke("save_azure_tts_config", {
        key: azureKey.trim(),
        region: azureRegion.trim(),
      });
      setAzureKey("");
      const c = await invoke<AzureConfigView>("get_azure_tts_config");
      setAzureRegion(c.region);
      setAzureHasKey(c.has_key);
      setAzureStatus({ text: t("voice.azureSaved"), ok: true });
    } catch (e) {
      setAzureStatus({ text: `${t("voice.azureSaveFailed")}: ${String(e)}`, ok: false });
    } finally {
      setAzureSaving(false);
    }
  }

  const disabled = project === null;
  const vc = project?.voice_config;

  return (
    <div className="space-y-4 text-sm">
      <div>
        <div className="font-medium">{t("voice.title")}</div>
        <div className="text-xs text-muted-foreground mt-0.5">{t("voice.titleHelper")}</div>
      </div>

      {/* English voice */}
      <VoiceSelect
        label={t("voice.enVoice")}
        value={vc?.en_voice ?? ""}
        voices={voices}
        disabled={disabled}
        onChange={(v) => project && onChange(patch(project, { en_voice: v }))}
      />

      {/* Chinese voice */}
      <VoiceSelect
        label={t("voice.zhVoice")}
        value={vc?.zh_voice ?? ""}
        voices={voices}
        disabled={disabled}
        onChange={(v) => project && onChange(patch(project, { zh_voice: v }))}
      />

      {/* Dialogue role voices */}
      <div className="space-y-2">
        <div className="text-xs font-medium text-muted-foreground">{t("voice.dialogRoles")}</div>
        <VoiceSelect
          label={t("voice.teacher")}
          value={vc?.teacher_voice ?? ""}
          voices={voices}
          disabled={disabled}
          onChange={(v) => project && onChange(patch(project, { teacher_voice: v }))}
        />
        <VoiceSelect
          label={t("voice.student")}
          value={vc?.student_voice ?? ""}
          voices={voices}
          disabled={disabled}
          onChange={(v) => project && onChange(patch(project, { student_voice: v }))}
        />
      </div>

      {/* Rate */}
      <div className="space-y-1">
        <label className="text-xs text-muted-foreground">
          {t("voice.rate")} ({vc ? (vc.rate >= 0 ? "+" : "") + vc.rate : "0"}%)
        </label>
        <input
          type="range"
          min={-50}
          max={50}
          step={5}
          value={vc?.rate ?? 0}
          disabled={disabled}
          onChange={(e) =>
            project && onChange(patch(project, { rate: Number(e.target.value) }))
          }
          className="w-full accent-primary disabled:opacity-50"
        />
      </div>

      {/* TTS 引擎 / Azure 付费接口 */}
      <div className="space-y-2 rounded border border-border p-2">
        <div className="text-xs font-medium">{t("voice.engineTitle")}</div>
        <div className="text-xs text-muted-foreground">
          {azureHasKey ? `✅ ${t("voice.azureActive")}` : t("voice.engineHelper")}
        </div>
        <div className="space-y-1">
          <label className="text-xs text-muted-foreground">{t("voice.azureRegion")}</label>
          <input
            type="text"
            value={azureRegion}
            onChange={(e) => setAzureRegion(e.target.value)}
            placeholder="eastasia"
            className="w-full rounded border border-input bg-background px-2 py-1 text-xs outline-none focus:border-ring"
          />
        </div>
        <div className="space-y-1">
          <label className="text-xs text-muted-foreground">{t("voice.azureKey")}</label>
          <input
            type="password"
            value={azureKey}
            onChange={(e) => setAzureKey(e.target.value)}
            placeholder={azureHasKey ? t("voice.azureKeyConfigured") : t("voice.azureKeyMissing")}
            autoComplete="new-password"
            className="w-full rounded border border-input bg-background px-2 py-1 text-xs outline-none focus:border-ring"
          />
        </div>
        <button
          onClick={saveAzure}
          disabled={azureSaving}
          className="w-full rounded border border-primary/40 px-2 py-1 text-xs bg-primary/10 text-primary hover:bg-primary/20 disabled:opacity-50 transition-colors"
        >
          {azureSaving ? t("voice.azureSaving") : t("voice.azureSave")}
        </button>
        {azureStatus && (
          <div
            className={`break-all rounded px-2 py-1 text-xs ${
              azureStatus.ok ? "bg-green-50 text-green-700" : "bg-red-50 text-red-600"
            }`}
          >
            {azureStatus.text}
          </div>
        )}
      </div>

      {/* divider */}
      <hr className="border-border" />

      {/* save status */}
      {saveStatus && (
        <div
          className={`break-all rounded px-2 py-1 text-xs ${
            saveStatus.ok
              ? "bg-green-50 text-green-700"
              : "bg-red-50 text-red-600"
          }`}
        >
          {saveStatus.text}
        </div>
      )}

      {/* generated files */}
      {generatedFiles.length > 0 && (
        <div className="space-y-1">
          <div className="font-medium text-green-700 text-xs">
            {t("voice.generatedTitle")} ({generatedFiles.length} {t("voice.generatedUnit")})
          </div>
          <ul className="space-y-0.5 text-xs">
            {generatedFiles.map((f) => (
              <li key={f} className="break-all text-muted-foreground">
                {f.split(/[\\/]/).pop()}
              </li>
            ))}
          </ul>
        </div>
      )}
    </div>
  );
}
