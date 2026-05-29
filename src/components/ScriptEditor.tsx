import { useCallback } from "react";
import type { Project, Part, Item } from "@/types";
import { cn } from "@/lib/utils";

// ── helpers ──────────────────────────────────────────────────────────────────

function updateItem(
  project: Project,
  partId: string,
  itemId: string,
  patch: Partial<Item>
): Project {
  return {
    ...project,
    parts: project.parts.map((p) =>
      p.id !== partId
        ? p
        : {
            ...p,
            items: p.items.map((it) =>
              it.id !== itemId ? it : { ...it, ...patch }
            ),
          }
    ),
  };
}

function updatePart(
  project: Project,
  partId: string,
  patch: Partial<Part>
): Project {
  return {
    ...project,
    parts: project.parts.map((p) =>
      p.id !== partId ? p : { ...p, ...patch }
    ),
  };
}

function moveItem(
  project: Project,
  partId: string,
  itemId: string,
  direction: "up" | "down"
): Project {
  return {
    ...project,
    parts: project.parts.map((p) => {
      if (p.id !== partId) return p;
      const items = [...p.items];
      const idx = items.findIndex((it) => it.id === itemId);
      if (idx === -1) return p;
      const swapIdx = direction === "up" ? idx - 1 : idx + 1;
      if (swapIdx < 0 || swapIdx >= items.length) return p;
      [items[idx], items[swapIdx]] = [items[swapIdx], items[idx]];
      return { ...p, items };
    }),
  };
}

// ── sub-components ────────────────────────────────────────────────────────────

interface ItemRowProps {
  item: Item;
  isFirst: boolean;
  isLast: boolean;
  onUpdate: (patch: Partial<Item>) => void;
  onMove: (dir: "up" | "down") => void;
}

function ItemRow({ item, isFirst, isLast, onUpdate, onMove }: ItemRowProps) {
  const GAP_OPTIONS: { value: number; label: string }[] = [
    { value: 500, label: "0.5s" },
    { value: 1000, label: "1s" },
    { value: 2000, label: "2s" },
    { value: 3000, label: "3s" },
    { value: 5000, label: "5s" },
  ];

  return (
    <div
      className={cn(
        "rounded border px-2 py-1.5 text-sm transition-opacity",
        !item.enabled && "opacity-40"
      )}
    >
      {/* row 1: enable + read_number + text + move buttons */}
      <div className="flex items-center gap-1.5">
        {/* enabled */}
        <input
          type="checkbox"
          title="启用此句"
          checked={item.enabled}
          onChange={(e) => onUpdate({ enabled: e.target.checked })}
          className="h-3.5 w-3.5 shrink-0 cursor-pointer accent-primary"
        />

        {/* read_number */}
        <input
          type="checkbox"
          title="朗读题号"
          checked={item.read_number}
          onChange={(e) => onUpdate({ read_number: e.target.checked })}
          className="h-3.5 w-3.5 shrink-0 cursor-pointer accent-primary"
        />
        <span className="shrink-0 text-xs text-muted-foreground">
          {item.number != null ? `#${item.number}` : "题号"}
        </span>

        {/* text input */}
        <input
          type="text"
          value={item.text}
          disabled={!item.enabled}
          onChange={(e) => onUpdate({ text: e.target.value })}
          className="min-w-0 flex-1 rounded border border-input bg-background px-1.5 py-0.5 text-sm outline-none focus:border-ring disabled:cursor-not-allowed disabled:opacity-50"
        />

        {/* move up */}
        <button
          title="上移"
          disabled={isFirst}
          onClick={() => onMove("up")}
          className="shrink-0 rounded px-1 py-0.5 text-xs hover:bg-muted disabled:pointer-events-none disabled:opacity-30"
        >
          ↑
        </button>
        {/* move down */}
        <button
          title="下移"
          disabled={isLast}
          onClick={() => onMove("down")}
          className="shrink-0 rounded px-1 py-0.5 text-xs hover:bg-muted disabled:pointer-events-none disabled:opacity-30"
        >
          ↓
        </button>
      </div>

      {/* row 2: repeat + gap_after_ms */}
      <div className="mt-1 flex items-center gap-2 pl-8">
        {/* repeat */}
        <div className="flex items-center gap-1">
          <span className="text-xs text-muted-foreground">读几遍</span>
          <button
            className={cn(
              "rounded border px-1.5 py-0.5 text-xs",
              item.repeat === 1
                ? "border-primary bg-primary/10 font-medium"
                : "border-border hover:bg-muted"
            )}
            onClick={() => onUpdate({ repeat: 1 })}
          >
            1
          </button>
          <button
            className={cn(
              "rounded border px-1.5 py-0.5 text-xs",
              item.repeat === 2
                ? "border-primary bg-primary/10 font-medium"
                : "border-border hover:bg-muted"
            )}
            onClick={() => onUpdate({ repeat: 2 })}
          >
            2
          </button>
        </div>

        {/* gap_after_ms */}
        <div className="flex items-center gap-1">
          <span className="text-xs text-muted-foreground">句后停顿</span>
          <select
            value={item.gap_after_ms}
            onChange={(e) => onUpdate({ gap_after_ms: Number(e.target.value) })}
            className="rounded border border-input bg-background px-1 py-0.5 text-xs outline-none focus:border-ring"
          >
            {GAP_OPTIONS.map((o) => (
              <option key={o.value} value={o.value}>
                {o.label}
              </option>
            ))}
          </select>
        </div>
      </div>
    </div>
  );
}

// ── PartBlock ─────────────────────────────────────────────────────────────────

interface PartBlockProps {
  part: Part;
  onUpdatePart: (patch: Partial<Part>) => void;
  onUpdateItem: (itemId: string, patch: Partial<Item>) => void;
  onMoveItem: (itemId: string, dir: "up" | "down") => void;
}

function PartBlock({
  part,
  onUpdatePart,
  onUpdateItem,
  onMoveItem,
}: PartBlockProps) {
  return (
    <div className="rounded-lg border bg-card p-3 shadow-sm">
      {/* Part header */}
      <div className="mb-2 space-y-1.5">
        {/* label (read-only display) + read_label checkbox */}
        <div className="flex items-center gap-2">
          <span className="font-semibold text-sm">{part.label}</span>
          <label className="ml-auto flex items-center gap-1 text-xs text-muted-foreground cursor-pointer select-none">
            <input
              type="checkbox"
              checked={part.read_label}
              onChange={(e) => onUpdatePart({ read_label: e.target.checked })}
              className="h-3.5 w-3.5 accent-primary"
            />
            朗读英文标题
          </label>
          <label className="flex items-center gap-1 text-xs text-muted-foreground cursor-pointer select-none">
            <input
              type="checkbox"
              checked={part.read_zh_instruction}
              onChange={(e) =>
                onUpdatePart({ read_zh_instruction: e.target.checked })
              }
              className="h-3.5 w-3.5 accent-primary"
            />
            朗读中文说明
          </label>
        </div>

        {/* zh_instruction editable */}
        <input
          type="text"
          value={part.zh_instruction ?? ""}
          placeholder="中文说明(可为空)"
          onChange={(e) =>
            onUpdatePart({
              zh_instruction: e.target.value === "" ? null : e.target.value,
            })
          }
          className="w-full rounded border border-input bg-background px-2 py-1 text-xs text-muted-foreground outline-none focus:border-ring placeholder:text-muted-foreground/50"
        />
      </div>

      {/* Items */}
      <div className="space-y-1.5">
        {part.items.map((item, idx) => (
          <ItemRow
            key={item.id}
            item={item}
            isFirst={idx === 0}
            isLast={idx === part.items.length - 1}
            onUpdate={(patch) => onUpdateItem(item.id, patch)}
            onMove={(dir) => onMoveItem(item.id, dir)}
          />
        ))}
      </div>
    </div>
  );
}

// ── ScriptEditor (exported) ───────────────────────────────────────────────────

interface ScriptEditorProps {
  project: Project;
  onChange: (updated: Project) => void;
}

export function ScriptEditor({ project, onChange }: ScriptEditorProps) {
  const handleUpdatePart = useCallback(
    (partId: string, patch: Partial<Part>) => {
      onChange(updatePart(project, partId, patch));
    },
    [project, onChange]
  );

  const handleUpdateItem = useCallback(
    (partId: string, itemId: string, patch: Partial<Item>) => {
      onChange(updateItem(project, partId, itemId, patch));
    },
    [project, onChange]
  );

  const handleMoveItem = useCallback(
    (partId: string, itemId: string, dir: "up" | "down") => {
      onChange(moveItem(project, partId, itemId, dir));
    },
    [project, onChange]
  );

  return (
    <div className="space-y-4">
      <div className="text-base font-medium">{project.title}</div>
      {project.parts.map((part) => (
        <PartBlock
          key={part.id}
          part={part}
          onUpdatePart={(patch) => handleUpdatePart(part.id, patch)}
          onUpdateItem={(itemId, patch) =>
            handleUpdateItem(part.id, itemId, patch)
          }
          onMoveItem={(itemId, dir) => handleMoveItem(part.id, itemId, dir)}
        />
      ))}
    </div>
  );
}
