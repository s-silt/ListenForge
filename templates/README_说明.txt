ListenForge 提取模板 / Extraction Templates
============================================

这些是 ListenForge 的 5 个"提取工作说明书"(给 AI 的指令),
决定 AI 怎么从练习卷里提取听力稿。每个模板一个 .txt,内容可直接用。
These are ListenForge's 5 built-in extraction prompts (instructions for the AI)
that decide how the listening script is pulled from a worksheet. One .txt per template.

怎么用 / How to use
-------------------
1. 应用里右下「提取模板」可直接切换这 5 个预设。
   In the app's "Extraction Templates" panel you can switch among these 5 presets.
2. 想自定义:打开对应 .txt → 复制全部内容 → 在「提取模板」编辑框里粘贴并修改 →
   点「另存为新模板」。下次"打开练习卷"就用你的模板。
   To customize: open a .txt, copy everything, paste into the editor, tweak it,
   then "Save as new template". It applies on the next worksheet you open.

文件 / Files
------------
01 标准听力卷 Standard   — 提听力原文、过滤答案、分中英、识别题型(默认)
                          Extract script, filter answers, split CN/EN (default)
02 通用英文朗读 General   — 任意英文文章/对话整段读,不滤答案、不分题型
                          Read any English text as a passage
03 单词听写默写 Words     — 单词 / 词组 / 句子 三种粒度,逐条提取保持完整
                          Words / phrases / sentences, kept intact
04 中英都读对照 Bilingual — 英文 + 中文翻译都读,中英对照
                          English + Chinese translation read together
05 对话分角色 Dialogue    — 对话型材料,识别说话人,老师声/学生声交替读
                          Dialogue with speakers → teacher/student voices

提示 / Note
-----------
这些 .txt 是当前内置模板的快照(对应代码里的预设)。
若以后内置模板更新,以应用「提取模板」里显示的为准。
These .txt files are a snapshot of the current built-in presets;
the app's panel is the source of truth if presets are updated later.
