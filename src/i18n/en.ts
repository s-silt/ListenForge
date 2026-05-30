const en = {
  // ── Top bar ────────────────────────────────────────────────────────────────
  topbar: {
    appName: "ListenForge",
    open: "Open Exercise",
    opening: "Extracting…",
    generate: "Generate Audio",
    generating: "Generating…",
    save: "Save Project",
    saving: "Saving…",
    langSwitch: "中",
    openHelper: "Pick a PDF / image / Word file to extract a listening script",
    generateHelper: "Synthesize the current script into MP3 audio files",
  },

  // ── Left panel (file info) ─────────────────────────────────────────────────
  fileInfo: {
    title: "File Info",
    noFile: "No file opened",
    created: "Created",
    parts: "Parts",
    items: "Items",
  },

  // ── Center empty state / onboarding ───────────────────────────────────────
  onboarding: {
    emptyHint: "👈 Click 'Open Exercise' (top-left) to pick a PDF / image / Word file",
    emptySub: "Supports PDF, DOCX, DOC, JPG, PNG, WEBP",
  },

  // ── Script editor ─────────────────────────────────────────────────────────
  editor: {
    enableItem: "Enable this line",
    readNumber: "Read question number",
    noNumber: "No #",
    moveUp: "Move up",
    moveDown: "Move down",
    repeatLabel: "Repeat",
    gapLabel: "Gap after",
    readLabel: "Read English title",
    readZhInstruction: "Read Chinese instruction",
    zhInstructionPlaceholder: "Chinese instruction (optional)",
  },

  // ── Voice settings ────────────────────────────────────────────────────────
  voice: {
    title: "Voice Settings",
    titleHelper: "Choose voices and speech rate; takes effect after opening a file",
    enVoice: "English Voice",
    zhVoice: "Chinese Voice",
    dialogRoles: "Dialogue Role Voices",
    teacher: "Teacher / Questioner",
    student: "Student / Responder",
    rate: "Rate",
    engineTitle: "TTS Engine",
    engineHelper: "Default is free (edge-tts, may be rate-limited on large batches). Enter an Azure paid Key to auto-switch to Azure — stable, no rate limit, same voices.",
    azureActive: "Azure paid TTS enabled (no rate limit)",
    azureRegion: "Azure Region",
    azureKey: "Azure Key",
    azureKeyConfigured: "Configured (leave blank to keep)",
    azureKeyMissing: "Paste Key 1 from your Azure Speech resource",
    azureSave: "Save Azure Config",
    azureSaving: "Saving…",
    azureSaved: "Saved — audio will use Azure from now on",
    azureSaveFailed: "Save failed",
    generatedTitle: "Audio generated",
    generatedUnit: "file(s)",
  },

  // ── AI settings ───────────────────────────────────────────────────────────
  ai: {
    title: "AI Settings",
    titleHelper: "First time? Fill in API URL / Model / Key and save",
    apiUrl: "API URL",
    model: "Model",
    apiKey: "API Key",
    keyConfigured: "Configured (leave blank to keep)",
    keyMissing: "Not set — please enter your Key",
    save: "Save Settings",
    saving: "Saving…",
    savedOk: "Settings saved",
    loadFailed: "Failed to load config",
    saveFailed: "Save failed",
    invalidUrl: "Invalid API URL — must start with http:// or https://",
    firstUseWarning: "First time? Fill in API URL / Model / Key and save before using",
  },

  // ── Template picker ───────────────────────────────────────────────────────
  template: {
    title: "Extraction Template",
    titleHelper: "Templates tell the AI how to parse the exercise format — select then Apply",
    selectLabel: "Select Template",
    builtinBadge: "(built-in)",
    contentLabel: "Template Content",
    builtinNote: "(built-in preset — edit and Save As to customise)",
    apply: "Apply Template",
    saveAs: "Save As New",
    saving: "Saving…",
    applying: "Applying…",
    applied: "Applied — takes effect on next open",
    savedAs: "Saved as",
    loadFailed: "Failed to load templates",
    applyFailed: "Apply failed",
    saveFailed: "Save failed",
    newNamePrompt: "New template name:",
    nameRequired: "Name cannot be empty",
    contentRequired: "Template content cannot be empty",
    contentTooLong: "Template content too long (max 20000 chars)",
    description:
      "Templates control how content is extracted (standard / general / vocabulary / bilingual / dialogue).\nAfter editing or switching, click 'Apply Template'; it takes effect on next open.",
  },
} as const;

export default en;
