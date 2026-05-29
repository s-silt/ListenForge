import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
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
      <div className="font-medium">语音设置</div>

      {/* 英文声音 */}
      <VoiceSelect
        label="英文声音"
        value={vc?.en_voice ?? ""}
        voices={voices}
        disabled={disabled}
        onChange={(v) => project && onChange(patch(project, { en_voice: v }))}
      />

      {/* 中文声音 */}
      <VoiceSelect
        label="中文声音"
        value={vc?.zh_voice ?? ""}
        voices={voices}
        disabled={disabled}
        onChange={(v) => project && onChange(patch(project, { zh_voice: v }))}
      />

      {/* 对话角色声音 */}
      <div className="space-y-2">
        <div className="text-xs font-medium text-muted-foreground">对话角色声音</div>
        <VoiceSelect
          label="老师 / 提问角色"
          value={vc?.teacher_voice ?? ""}
          voices={voices}
          disabled={disabled}
          onChange={(v) => project && onChange(patch(project, { teacher_voice: v }))}
        />
        <VoiceSelect
          label="学生 / 回答角色"
          value={vc?.student_voice ?? ""}
          voices={voices}
          disabled={disabled}
          onChange={(v) => project && onChange(patch(project, { student_voice: v }))}
        />
      </div>

      {/* 语速 */}
      <div className="space-y-1">
        <label className="text-xs text-muted-foreground">
          语速 ({vc ? (vc.rate >= 0 ? "+" : "") + vc.rate : "0"}%)
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
            音频生成完成 ({generatedFiles.length} 文件)
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
