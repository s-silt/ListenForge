export type SourceType = "pdf_text" | "pdf_scanned" | "docx" | "image";

export type TaskType =
  | "listen_and_choose"
  | "listen_and_number"
  | "listen_and_judge"
  | "listen_and_write"
  | "listen_and_circle"
  | "listen_passage"
  | "unknown";

export interface Item {
  id: string;
  number: number | null;
  text: string;
  enabled: boolean;
  repeat: number;
  gap_after_ms: number;
  read_number: boolean;
  override_voice: string | null;
  speaker: string | null;
}

export interface Part {
  id: string;
  index: number;
  label: string;
  task_type: TaskType;
  read_label: boolean;
  zh_instruction: string | null;
  read_zh_instruction: boolean;
  items: Item[];
  gap_after_ms: number;
}

export interface VoiceConfig {
  provider: string;
  en_voice: string;
  zh_voice: string;
  rate: number;
  pitch: number;
  volume: number;
  teacher_voice: string;
  student_voice: string;
}

export interface ExportConfig {
  output_dir: string;
  generate_full: boolean;
  generate_per_part: boolean;
  generate_script_txt: boolean;
  generate_script_docx: boolean;
  generate_ssml: boolean;
  zip_all: boolean;
}

export interface Project {
  id: string;
  title: string;
  source_file: string;
  source_type: SourceType;
  created_at: string;
  parts: Part[];
  voice_config: VoiceConfig;
  export_config: ExportConfig;
}

export interface ProgressPayload {
  current: number;
  total: number;
  message: string;
}

export interface LlmConfigView {
  base_url: string;
  model: string;
  has_api_key: boolean;
}
