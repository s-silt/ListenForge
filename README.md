# ListenForge · 英语听力音频生成工具

> **上传一张英语练习卷,几分钟生成接近真人朗读的听力 MP3。**
> *Upload an English worksheet, get natural teacher-style listening MP3 in minutes.*

中英双语界面 · 自动识别听力原文 · 过滤答案 · 老师朗读风格(报题号 / 停顿 / 重复)
*Bilingual UI · auto-detect listening script · filters answer keys · teacher-style reading.*

---

## 📖 这是什么

ListenForge 是一个 **Windows 桌面工具**,面向**小学英语老师和家长**。

你上传一份英语练习卷(PDF / 图片 / Word),它会:
1. **自动找出"听力原文"**(不是整张卷子,是要朗读的那部分)
2. **过滤掉答案**(`1.B 2.A`、`答案:` 这些不会被读)
3. **区分中英文**(中文说明 vs 英文句子)
4. 按**老师朗读风格**生成 MP3(报题号 "Number one." → 停顿 → 句子 → 重复)

它**不是**普通的 TTS 工具,而是围绕"听力题制作流程"设计的:自动提取、可编辑、真人感声音、多种题型、对话分角色。

### What is it
ListenForge is a **Windows desktop tool** for **primary-school English teachers and parents**. Upload a worksheet (PDF/image/Word); it auto-detects the *listening script*, filters out answer keys, separates Chinese/English, and generates teacher-style MP3 (item number → pause → sentence → repeat). It's built around the listening-exam workflow — not a plain TTS.

---

## ✨ 功能 / Features

| 功能 | Feature |
|---|---|
| 🔍 AI 自动提取听力原文,过滤答案、分中英 | AI auto-extract, filters answers, splits CN/EN |
| ✏️ 可编辑:改句子、调重复/停顿、勾选要不要读 | Editable: text, repeat, pause, toggle |
| 🗣️ 7 种真人感声音(英式/美式/儿童/中文女声) | 7 natural voices (UK/US/child/Chinese) |
| 🎭 对话分角色朗读(老师声 / 学生声 自动切换) | Dialogue roles (teacher/student voices) |
| 📋 5 种提取模板可切换 + 自定义 | 5 switchable + custom templates |
| 🌐 中英双语界面 | Bilingual UI (中文 / English) |
| 🎵 导出完整 MP3 + 分大题 MP3 | Full + per-part MP3 export |

---

## 💻 系统要求 / Requirements

- **Windows 11 (ARM64)** — 本发行版为 ARM64 架构 / this build is ARM64
- WebView2(Windows 11 已内置 / built into Win11)
- 联网 / Internet(提取走 AI、语音合成走微软 edge-tts)
- 一个 **OpenAI 兼容 API 的 Key**(自备)/ an OpenAI-compatible API key

---

## 📥 安装 / Installation

### 方式一:安装版(推荐) / Installer (recommended)
下载 `listenforge_x.x.x_arm64-setup.exe` → 双击 → 一路下一步 → 开始菜单 / 桌面出现 **ListenForge** 图标。
*Download the setup `.exe`, double-click to install, then launch from Start menu.*

### 方式二:绿色免安装版 / Portable
下载绿色版压缩包 → 解压到任意文件夹 → 双击 `listenforge.exe`。
**注意**:`pdfium.dll` 必须和 `listenforge.exe` 在同一文件夹。
*Extract the portable zip anywhere and double-click `listenforge.exe` (keep `pdfium.dll` next to it).*

---

## ⚙️ 首次配置(重要)/ First-time Setup

第一次打开,需要在 **右下角「AI 设置」** 填三样东西,然后点「保存」:

1. **API 地址 / API URL** — 你的 OpenAI 兼容接口地址(通常以 `/v1` 结尾)
2. **模型 / Model** — 例如 `gpt-5.4-mini`(填你的服务支持的)
3. **API Key** — 你的密钥(只存在本机,不会上传到任何代码仓库)

> 没填 Key 时,界面顶部会有橙色提示。Key 保存在本机 `文档\ListenForge\.env`,**不会进 GitHub**。
> *Fill API URL / Model / Key in "AI Settings" (bottom-right) and Save. The key stays local and never enters the repo.*

---

## 🚀 怎么用 / How to Use

1. 点左上 **「打开练习卷」**,选一份 PDF / 图片 / Word
   *Click "Open Worksheet", pick a PDF/image/Word*
2. 等几秒,**中栏出现听力稿**(已自动提取、过滤答案、分好中英)
   *Wait — the script appears in the middle, answers filtered*
3. **核对一下**,需要的话直接改(改句子、调重复次数/停顿、勾掉不读的句子)
   *Review and edit if needed*
4. (可选)右下选 **提取模板** / 右侧选 **声音**
   *Optionally pick a template / voices*
5. 点 **「生成音频」**,选一个输出文件夹
   *Click "Generate Audio", choose an output folder*
6. 去那个文件夹,听 **`xxx_full.mp3`**(完整版)或分大题的 MP3
   *Open the folder and listen to `xxx_full.mp3`*

---

## 📋 提取模板 / Extraction Templates

可在右下「提取模板」切换,也能改了**另存为**自己的模板:

| 模板 / Template | 用途 / Use |
|---|---|
| **标准听力卷** / Standard | 提听力原文、滤答案、分中英(默认) |
| **通用英文朗读** / General | 任意英文文章/对话,整段读 |
| **单词听写** / Words | 逐词朗读,适合默写 |
| **中英都读** / Bilingual | 中文说明 + 英文都读,中英对照 |
| **对话分角色** / Dialogue | 对话型材料,老师声 / 学生声交替读 |

---

## 🛠️ 技术栈 / Tech Stack

[Tauri 2](https://tauri.app) · React 19 · Rust · [pdfium](https://pdfium.googlesource.com/pdfium/)(PDF 文本提取)· OpenAI 兼容 API(智能提取)· 微软 edge-tts(语音合成)

---

## ❓ 常见问题 / FAQ

**Q: 提取得不准 / 漏句子?**
A: AI 偶尔会判断错——直接在中栏编辑,或在「提取模板」里换个模板/改 prompt 重新打开练习卷。
*The AI isn't perfect — edit in the middle panel, or switch/tweak the template and re-open.*

**Q: 启动报"加载 pdfium 库失败"?**
A: `pdfium.dll` 没和 exe 放一起(绿色版)。把它放到 `listenforge.exe` 同目录。
*Keep `pdfium.dll` next to `listenforge.exe`.*

**Q: 我的卷子是扫描版/图片?**
A: 也支持——会自动渲染成图片走 AI 视觉识别(需要你的 API 支持图片输入)。
*Scanned/image worksheets are supported via vision (requires an image-capable API).*

---

## 📄 许可 / License

个人自用项目 / Personal project.
