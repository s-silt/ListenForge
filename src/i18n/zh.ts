const zh = {
  // ── Top bar ────────────────────────────────────────────────────────────────
  topbar: {
    appName: "ListenForge",
    open: "打开练习卷",
    opening: "提取中…",
    generate: "生成音频",
    generating: "生成中…",
    save: "保存项目",
    saving: "保存中…",
    langSwitch: "EN",
    openHelper: "选一份 PDF / 图片 / Word 文件提取听力稿",
    generateHelper: "把当前听力稿合成为 MP3 音频文件",
  },

  // ── Left panel (file info) ─────────────────────────────────────────────────
  fileInfo: {
    title: "文件信息",
    noFile: "未打开文件",
    created: "创建",
    parts: "大题",
    items: "小题",
  },

  // ── Center empty state / onboarding ───────────────────────────────────────
  onboarding: {
    emptyHint: "👈 点左上『打开练习卷』选一份 PDF / 图片 / Word 开始",
    emptySub: "支持 PDF、DOCX、DOC、JPG、PNG、WEBP 格式",
  },

  // ── Script editor ─────────────────────────────────────────────────────────
  editor: {
    enableItem: "启用此句",
    readNumber: "朗读题号",
    noNumber: "题号",
    moveUp: "上移",
    moveDown: "下移",
    repeatLabel: "读几遍",
    gapLabel: "句后停顿",
    readLabel: "朗读英文标题",
    readZhInstruction: "朗读中文说明",
    zhInstructionPlaceholder: "中文说明(可为空)",
  },

  // ── Voice settings ────────────────────────────────────────────────────────
  voice: {
    title: "语音设置",
    titleHelper: "选择合成音色和语速,打开文件后生效",
    enVoice: "英文声音",
    zhVoice: "中文声音",
    dialogRoles: "对话角色声音",
    teacher: "老师 / 提问角色",
    student: "学生 / 回答角色",
    rate: "语速",
    engineTitle: "语音引擎 (TTS)",
    engineHelper: "默认免费(edge-tts,大批量可能被限流)。填入 Azure 付费 Key 后自动改用 Azure,稳定无限流、音色不变。",
    azureActive: "已启用 Azure 付费语音(无限流)",
    azureRegion: "Azure 区域 (Region)",
    azureKey: "Azure 密钥 (Key)",
    azureKeyConfigured: "已配置(留空不改)",
    azureKeyMissing: "粘贴 Azure 语音服务的「密钥1」",
    azureSave: "保存 Azure 配置",
    azureSaving: "保存中…",
    azureSaved: "已保存,之后生成音频自动用 Azure",
    azureSaveFailed: "保存失败",
    generatedTitle: "音频生成完成",
    generatedUnit: "文件",
  },

  // ── AI settings ───────────────────────────────────────────────────────────
  ai: {
    title: "AI 设置",
    titleHelper: "首次使用:请先填写 API 地址 / 模型 / Key 并保存",
    apiUrl: "API 地址",
    model: "模型",
    apiKey: "API Key",
    keyConfigured: "已配置(留空不改)",
    keyMissing: "未配置,请填入 Key",
    save: "保存设置",
    saving: "保存中…",
    savedOk: "设置已保存",
    loadFailed: "加载配置失败",
    saveFailed: "保存失败",
    invalidUrl: "API 地址格式不对,应以 http:// 或 https:// 开头",
    firstUseWarning: "首次使用:请先填写 API 地址 / 模型 / Key 并保存",
  },

  // ── Template picker ───────────────────────────────────────────────────────
  template: {
    title: "提取模板",
    titleHelper: "模板决定 AI 怎么理解卷子格式,切换后点「应用」",
    selectLabel: "选择模板",
    builtinBadge: "(预设)",
    contentLabel: "模板内容",
    builtinNote: "(内置预设,可编辑后另存)",
    apply: "应用此模板",
    saveAs: "另存为新模板",
    saving: "保存中…",
    applying: "应用中…",
    applied: "已应用,下次打开练习卷生效",
    savedAs: "已另存为",
    loadFailed: "加载模板失败",
    applyFailed: "应用失败",
    saveFailed: "保存失败",
    newNamePrompt: "新模板名称:",
    nameRequired: "模板名称不能为空",
    contentRequired: "模板内容不能为空",
    contentTooLong: "模板内容过长(上限 20000 字符)",
    description:
      '模板决定“怎么从卷子提取”(标准 / 通用 / 单词 / 中英对照 / 对话)。\n改了或切换模板后点「应用此模板」,下次打开练习卷时生效。',
  },
} as const;

export default zh;
