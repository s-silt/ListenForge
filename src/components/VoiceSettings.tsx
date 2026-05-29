import type { Project, VoiceConfig } from "@/types";

const EN_VOICES = [
  "en-US-AriaNeural",
  "en-US-GuyNeural",
  "en-US-JennyNeural",
  "en-GB-LibbyNeural",
  "en-GB-RyanNeural",
  "en-AU-NatashaNeural",
];

const ZH_VOICES = [
  "zh-CN-XiaoxiaoNeural",
  "zh-CN-YunxiNeural",
  "zh-CN-XiaohanNeural",
  "zh-CN-YunjianNeural",
  "zh-TW-HsiaoChenNeural",
];

interface VoiceSettingsProps {
  project: Project;
  onChange: (updated: Project) => void;
  generatedFiles: string[];
  saveStatus: string;
}

function patch(project: Project, vc: Partial<VoiceConfig>): Project {
  return { ...project, voice_config: { ...project.voice_config, ...vc } };
}

export function VoiceSettings({
  project,
  onChange,
  generatedFiles,
  saveStatus,
}: VoiceSettingsProps) {
  const vc = project.voice_config;

  return (
    <div className="space-y-4 text-sm">
      <div className="font-medium">语音设置</div>

      {/* en_voice */}
      <div className="space-y-1">
        <label className="text-xs text-muted-foreground">英文声音</label>
        <select
          value={vc.en_voice}
          onChange={(e) => onChange(patch(project, { en_voice: e.target.value }))}
          className="w-full rounded border border-input bg-background px-2 py-1 text-xs outline-none focus:border-ring"
        >
          {EN_VOICES.map((v) => (
            <option key={v} value={v}>
              {v}
            </option>
          ))}
          {/* keep current if not in list */}
          {!EN_VOICES.includes(vc.en_voice) && (
            <option value={vc.en_voice}>{vc.en_voice}</option>
          )}
        </select>
      </div>

      {/* zh_voice */}
      <div className="space-y-1">
        <label className="text-xs text-muted-foreground">中文声音</label>
        <select
          value={vc.zh_voice}
          onChange={(e) => onChange(patch(project, { zh_voice: e.target.value }))}
          className="w-full rounded border border-input bg-background px-2 py-1 text-xs outline-none focus:border-ring"
        >
          {ZH_VOICES.map((v) => (
            <option key={v} value={v}>
              {v}
            </option>
          ))}
          {!ZH_VOICES.includes(vc.zh_voice) && (
            <option value={vc.zh_voice}>{vc.zh_voice}</option>
          )}
        </select>
      </div>

      {/* rate */}
      <div className="space-y-1">
        <label className="text-xs text-muted-foreground">
          语速 ({vc.rate >= 0 ? "+" : ""}{vc.rate}%)
        </label>
        <input
          type="range"
          min={-50}
          max={50}
          step={5}
          value={vc.rate}
          onChange={(e) => onChange(patch(project, { rate: Number(e.target.value) }))}
          className="w-full accent-primary"
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
