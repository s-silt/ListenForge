import { memo, useCallback, useRef } from "react";
import { useTranslation } from "react-i18next";
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

// 模块级常量：不依赖 props/t，避免每行每次 render 重建
const GAP_OPTIONS: { value: number; label: string }[] = [
  { value: 500, label: "0.5s" },
  { value: 1000, label: "1s" },
  { value: 2000, label: "2s" },
  { value: 3000, label: "3s" },
  { value: 5000, label: "5s" },
];

interface ItemRowProps {
  item: Item;
  isFirst: boolean;
  isLast: boolean;
  // 接收 itemId 的稳定回调（由 PartBlock 提供），配合 memo 避免逐键重渲整列
  onUpdate: (itemId: string, patch: Partial<Item>) => void;
  onMove: (itemId: string, dir: "up" | "down") => void;
}

// memo：仅当本行自身 props（item 引用、首/末标志、稳定回调）变化时才重渲。
// 编辑某一行的文本只会让该行的 item 引用变化 → 只重渲这一行。
const ItemRow = memo(function ItemRow({ item, isFirst, isLast, onUpdate, onMove }: ItemRowProps) {
  const { t } = useTranslation();

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
          title={t("editor.enableItem")}
          checked={item.enabled}
          onChange={(e) => onUpdate(item.id, { enabled: e.target.checked })}
          className="h-3.5 w-3.5 shrink-0 cursor-pointer accent-primary"
        />

        {/* read_number */}
        <input
          type="checkbox"
          title={t("editor.readNumber")}
          checked={item.read_number}
          onChange={(e) => onUpdate(item.id, { read_number: e.target.checked })}
          className="h-3.5 w-3.5 shrink-0 cursor-pointer accent-primary"
        />
        <span className="shrink-0 text-xs text-muted-foreground">
          {item.number != null ? `#${item.number}` : t("editor.noNumber")}
        </span>

        {/* text input */}
        <input
          type="text"
          value={item.text}
          disabled={!item.enabled}
          onChange={(e) => onUpdate(item.id, { text: e.target.value })}
          className="min-w-0 flex-1 rounded border border-input bg-background px-1.5 py-0.5 text-sm outline-none focus:border-ring disabled:cursor-not-allowed disabled:opacity-50"
        />

        {/* move up */}
        <button
          title={t("editor.moveUp")}
          disabled={isFirst}
          onClick={() => onMove(item.id, "up")}
          className="shrink-0 rounded px-1 py-0.5 text-xs hover:bg-muted disabled:pointer-events-none disabled:opacity-30"
        >
          ↑
        </button>
        {/* move down */}
        <button
          title={t("editor.moveDown")}
          disabled={isLast}
          onClick={() => onMove(item.id, "down")}
          className="shrink-0 rounded px-1 py-0.5 text-xs hover:bg-muted disabled:pointer-events-none disabled:opacity-30"
        >
          ↓
        </button>
      </div>

      {/* row 2: repeat + gap_after_ms */}
      <div className="mt-1 flex items-center gap-2 pl-8">
        {/* repeat */}
        <div className="flex items-center gap-1">
          <span className="text-xs text-muted-foreground">{t("editor.repeatLabel")}</span>
          <button
            className={cn(
              "rounded border px-1.5 py-0.5 text-xs",
              item.repeat === 1
                ? "border-primary bg-primary/10 font-medium"
                : "border-border hover:bg-muted"
            )}
            onClick={() => onUpdate(item.id, { repeat: 1 })}
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
            onClick={() => onUpdate(item.id, { repeat: 2 })}
          >
            2
          </button>
        </div>

        {/* gap_after_ms */}
        <div className="flex items-center gap-1">
          <span className="text-xs text-muted-foreground">{t("editor.gapLabel")}</span>
          <select
            value={item.gap_after_ms}
            onChange={(e) => onUpdate(item.id, { gap_after_ms: Number(e.target.value) })}
            className="rounded border border-input bg-background px-1 py-0.5 text-xs outline-none focus:border-ring"
          >
            {GAP_OPTIONS.map((o) => (
              <option key={o.value} value={o.value}>
                {o.label}
              </option>
            ))}
            {/* keep current value if not one of the presets（否则 select 显示空白） */}
            {!GAP_OPTIONS.some((o) => o.value === item.gap_after_ms) && (
              <option value={item.gap_after_ms}>
                {(item.gap_after_ms / 1000).toString()}s
              </option>
            )}
          </select>
        </div>
      </div>
    </div>
  );
});

// ── PartBlock ─────────────────────────────────────────────────────────────────

interface PartBlockProps {
  part: Part;
  // 接收 partId 的稳定回调（由 ScriptEditor 提供）
  onUpdatePart: (partId: string, patch: Partial<Part>) => void;
  onUpdateItem: (partId: string, itemId: string, patch: Partial<Item>) => void;
  onMoveItem: (partId: string, itemId: string, dir: "up" | "down") => void;
}

// memo：仅当本 part 引用或上游稳定回调变化时才重渲。编辑某 part 内一行，
// 只有该 part 的引用会变 → 其它 PartBlock 直接跳过重渲。
const PartBlock = memo(function PartBlock({
  part,
  onUpdatePart,
  onUpdateItem,
  onMoveItem,
}: PartBlockProps) {
  const { t } = useTranslation();
  const partId = part.id;

  // part 作用域内的稳定回调：传给子组件后，ItemRow 的 memo 才能生效
  const updateThisPart = useCallback(
    (patch: Partial<Part>) => onUpdatePart(partId, patch),
    [onUpdatePart, partId]
  );
  const updateItemInPart = useCallback(
    (itemId: string, patch: Partial<Item>) => onUpdateItem(partId, itemId, patch),
    [onUpdateItem, partId]
  );
  const moveItemInPart = useCallback(
    (itemId: string, dir: "up" | "down") => onMoveItem(partId, itemId, dir),
    [onMoveItem, partId]
  );

  return (
    <div className="rounded border bg-card p-3">
      {/* Part header */}
      <div className="mb-2 space-y-1.5">
        {/* label (read-only display) + read_label checkbox */}
        <div className="flex items-center gap-2">
          <span className="font-semibold text-sm">{part.label}</span>
          <label className="ml-auto flex items-center gap-1 text-xs text-muted-foreground cursor-pointer select-none">
            <input
              type="checkbox"
              checked={part.read_label}
              onChange={(e) => updateThisPart({ read_label: e.target.checked })}
              className="h-3.5 w-3.5 accent-primary"
            />
            {t("editor.readLabel")}
          </label>
          <label className="flex items-center gap-1 text-xs text-muted-foreground cursor-pointer select-none">
            <input
              type="checkbox"
              checked={part.read_zh_instruction}
              onChange={(e) =>
                updateThisPart({ read_zh_instruction: e.target.checked })
              }
              className="h-3.5 w-3.5 accent-primary"
            />
            {t("editor.readZhInstruction")}
          </label>
        </div>

        {/* zh_instruction editable */}
        <input
          type="text"
          value={part.zh_instruction ?? ""}
          placeholder={t("editor.zhInstructionPlaceholder")}
          onChange={(e) =>
            updateThisPart({
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
            onUpdate={updateItemInPart}
            onMove={moveItemInPart}
          />
        ))}
      </div>
    </div>
  );
});

// ── ScriptEditor (exported) ───────────────────────────────────────────────────

interface ScriptEditorProps {
  project: Project;
  onChange: (updated: Project) => void;
}

export function ScriptEditor({ project, onChange }: ScriptEditorProps) {
  // 用 ref 持有最新 project / onChange，使下面的回调身份稳定（deps []），
  // 从而让 PartBlock / ItemRow 的 React.memo 真正生效（回调不再随每次击键重建）。
  // 回调在用户事件时才触发，此时读到的 *Ref.current 已是最新一次 render 的值，无 stale 风险。
  const projectRef = useRef(project);
  projectRef.current = project;
  const onChangeRef = useRef(onChange);
  onChangeRef.current = onChange;

  const handleUpdatePart = useCallback(
    (partId: string, patch: Partial<Part>) => {
      onChangeRef.current(updatePart(projectRef.current, partId, patch));
    },
    []
  );

  const handleUpdateItem = useCallback(
    (partId: string, itemId: string, patch: Partial<Item>) => {
      onChangeRef.current(updateItem(projectRef.current, partId, itemId, patch));
    },
    []
  );

  const handleMoveItem = useCallback(
    (partId: string, itemId: string, dir: "up" | "down") => {
      onChangeRef.current(moveItem(projectRef.current, partId, itemId, dir));
    },
    []
  );

  return (
    <div className="space-y-4">
      <div className="text-base font-medium">{project.title}</div>
      {project.parts.map((part) => (
        <PartBlock
          key={part.id}
          part={part}
          onUpdatePart={handleUpdatePart}
          onUpdateItem={handleUpdateItem}
          onMoveItem={handleMoveItem}
        />
      ))}
    </div>
  );
}
