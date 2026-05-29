import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useTranslation } from "react-i18next";
import type { Project, VoiceConfig, Voice } from "@/types";

interface VoiceSettingsProps {
  project: Project | null;
  onChange: (updated: Project) => void;
  generatedFiles: string[];
  saveStatus: string;
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

  useEffect(() => {
    invoke<Voice[]>("get_voices")
      .then(setVoices)
      .catch(() => {
        // Fallback: leave voices empty; dropdowns will still show current value
      });
  }, []);

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

      {/* divider */}
      <hr className="border-border" />

      {/* save status */}
      {saveStatus && (
        <div className="break-all rounded bg-muted px-2 py-1 text-xs text-muted-foreground">
          {saveStatus}
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
