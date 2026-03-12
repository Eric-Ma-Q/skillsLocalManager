export type Locale = "zh" | "en";

export const DEFAULT_LOCALE: Locale = "zh";
export const LOCALE_STORAGE_KEY = "skillsLocalManager.locale";

const messages = {
  zh: {
    appName: "skillsLocalManager",
    appSubtitle: "技能管理器",
    skillDotsHint: "灰点=已安装代理终端数",
    general: "通用",
    allSkills: "全部技能",
    browseRegistry: "浏览技能仓库",
    installedAgents: "已检测代理",
    settings: "设置",
    registrySearchPlaceholder: "搜索技能...",
    totalCount: (count: number) => `共 ${count} 项`,
    skillsDirectory: "技能目录",
    configDirectory: "配置目录",
    globalSharedSkills: "全局共享技能",
    about: "关于",
    version: "版本",
    platform: "平台",
    registry: "仓库",
    failedToLoadRegistry: "加载仓库失败",
    retry: "重试",
    noResultsFound: "未找到结果",
    tryDifferentSearch: "请尝试其他搜索关键词。",
    noSkillsFound: "未找到技能",
    noSkillsDescription: "当前代理暂未安装技能。可以前往技能仓库浏览并安装新技能。",
    backToLibrary: "返回技能库",
    updateSkill: "更新技能",
    verifiedSkill: "已验证技能",
    terminalVersions: "终端版本",
    sameVersionAcrossTerminals: "各终端版本一致",
    multipleTerminalVersionsDetected: "检测到多个终端版本",
    installation: "安装管理",
    coverToAgent: "覆盖到此终端",
    coverToAllAvailable: "覆盖到所有可用终端",
    coveringToAll: "覆盖中...",
    coveringToAgent: "覆盖中...",
    coverHistory: "历史回退",
    rollbackToThisVersion: "回退到此版本",
    loadingHistory: "加载历史中...",
    noCoverHistory: "暂无覆盖历史",
    coverOperationSummary: (applied: number, skipped: number) => `覆盖完成：成功 ${applied}，跳过 ${skipped}`,
    rollbackSuccess: "回退成功",
    inheritedVersion: "继承版本",
    remove: "移除",
    install: "安装",
    enabled: "已启用",
    skillId: "技能 ID",
    lastScanned: "上次扫描",
    minutesAgo: (minutes: number) => `${minutes} 分钟前`,
    loadingRegistryFallback: "加载仓库失败",
    searchFailedFallback: "搜索失败",
    operationFailedPrefix: "操作失败",
    agentsCount: (count: number) => `${count} 个代理`,
    language: "语言",
    chinese: "cn",
    english: "en",
    translatePromptToChinese: "翻译中文",
    translatePromptToEnglish: "翻译英文",
    translating: "翻译中...",
    translatedPrompt: "中文 Prompt",
    translatedPromptEnglish: "English Prompt",
    machineTranslationDisclaimer: "机器翻译，仅供参考。",
    translationFailedPrefix: "翻译失败",
    translatorSettingsTitle: "硅基流动翻译配置",
    translatorSettingsDescription: "使用大模型翻译 prompt。配置后会保存在本地。",
    translatorKeyLabel: "SiliconFlow API Key",
    translatorModelLabel: "模型",
    translatorBaseUrlLabel: "Base URL（固定）",
    saveTranslatorConfig: "保存翻译配置",
    saving: "保存中...",
    translatorConfigSaved: "翻译配置已保存",
    translatorConfigSaveFailed: "保存翻译配置失败",
    testTranslatorConnection: "连通性测试",
    testingConnection: "测试中...",
    translatorConnectionSuccess: "连通成功",
    translatorConnectionFailed: "连通失败",
    openTranslatorGuide: "打开配置指引",
    missingTranslatorConfigPrompt: "尚未配置硅基流动 API Key。是否前往设置页填写？",
    openGuidePrompt: "是否同时打开硅基流动官方指引？",
    translatorNoteTitle: "备注",
    translatorNoteBody: "模型翻译使用硅基流动接口。Base URL 固定为 https://api.siliconflow.cn/v1，且默认关闭 thinking。",
    liveTranslationTitle: "实时翻译",
    liveTranslationHint: "正在接收流式返回内容...",
    liveTranslationRunning: "翻译进行中",
    liveTranslationDone: "翻译完成",
    liveTranslationError: "翻译中断",
    closeModal: "关闭",
    translatorLogsTitle: "翻译日志",
    translatorLogPath: "日志文件",
    refreshLogs: "刷新日志",
    openLogFile: "打开日志文件",
    openDirectory: "打开目录",
    loadingLogs: "加载日志中...",
    noLogsYet: "暂无日志，先执行一次连通性测试或翻译。",
    claudeInitBannerTitle: "建议先初始化 Claude Skills",
    claudeInitBannerExistingBody: "检测到 Claude 技能目录。可以一键安装推荐 starter skills，安装后会立即出现在当前技能列表中。",
    claudeInitBannerMissingBody: "未发现 Claude 技能目录。可以先创建 ~/.claude/skills，再把推荐 starter skills 下载到本地。",
    claudeInitBannerAction: "初始化 Claude Skills",
    claudeInitBannerLater: "稍后",
    claudeInitBannerDocs: "查看官方说明",
    claudeInitModalTitle: "Claude Skills 初始化",
    claudeInitModalSubtitle: "本地目录优先：把 Anthropic 官方 skills 安装到 ~/.claude/skills，确保 SkillDeck 可立即扫描。",
    claudeInitTargetDir: "目标目录",
    claudeInitSourceRepo: "来源仓库",
    claudeInitRecommended: "推荐 starter skills",
    claudeInitOptional: "可选文档 skills",
    claudeInitSelectionHint: "选择要安装的 skills",
    claudeInitCreateAndInstall: "创建目录并安装",
    claudeInitInstall: "安装到 Claude Skills",
    claudeInitInstallRunning: "安装中...",
    claudeInitClose: "关闭",
    claudeInitOpenDocs: "打开 Claude 官方说明",
    claudeInitLoading: "正在读取 Claude 初始化信息...",
    claudeInitCliMissingNotice: "未检测到 Claude CLI。不会阻止本地初始化，但安装完成后仍建议补装 Claude Code。",
    claudeInitTargetMissingNotice: "未发现 ~/.claude/skills。安装时会先自动创建目录。",
    claudeInitTargetReadyNotice: "已检测到 ~/.claude/skills，可直接安装推荐包。",
    claudeInitCannotCreateNotice: "当前无法确认 ~/.claude/skills 可创建，请先检查家目录权限。",
    claudeInitExistingSkillsNotice: (count: number) => `目标目录中已存在 ${count} 个同名 skill，安装时将自动跳过。`,
    claudeInitExistingSkillTag: "已存在",
    claudeInitNoSkillsSelected: "请至少选择一个 skill",
    claudeInitInstalledSummary: (installed: number, skipped: number) => `初始化完成：安装 ${installed}，跳过 ${skipped}`,
    claudeInitInstalledList: "已安装",
    claudeInitSkippedList: "已跳过",
    agentStatusCli: "CLI 可用",
    agentStatusCliDescription: "检测到命令行入口，可直接通过 CLI 使用。",
    agentStatusSkills: "技能目录可用",
    agentStatusSkillsDescription: "未检测到 CLI，但已发现技能目录或本地技能。",
    agentStatusConfig: "配置已发现",
    agentStatusConfigDescription: "已发现配置目录，可继续接入技能目录。",
    agentStatusMissing: "未检测到",
    agentStatusMissingDescription: "未发现 CLI、配置目录或技能目录。",
    statusSkillCount: (count: number) => `${count} 个本地技能`,
    cursorSkillReadNoteTitle: "Cursor 兼容读取说明",
    cursorSkillReadNoteBody: "除 Cursor 自身技能目录外，还会按兼容规则读取以下目录中的技能。",
    sourceAgentLabel: "来源",
    pathExists: "已发现",
    pathMissing: "未发现",
    sameVersion: "相同版本",
    specificVersion: "指定版本",
    activeVersionPrefix: "当前",
    skillDivergedTag: "版本分歧",
  },
  en: {
    appName: "skillsLocalManager",
    appSubtitle: "Skill Manager",
    skillDotsHint: "Gray dots = installed agent terminals",
    general: "General",
    allSkills: "All Skills",
    browseRegistry: "Browse Registry",
    installedAgents: "Detected Agents",
    settings: "Settings",
    registrySearchPlaceholder: "Search skills...",
    totalCount: (count: number) => `${count} Total`,
    skillsDirectory: "Skills Directory",
    configDirectory: "Config Directory",
    globalSharedSkills: "Global shared skills",
    about: "About",
    version: "Version",
    platform: "Platform",
    registry: "Registry",
    failedToLoadRegistry: "Failed to load registry",
    retry: "Retry",
    noResultsFound: "No results found",
    tryDifferentSearch: "Try a different search term.",
    noSkillsFound: "No skills found",
    noSkillsDescription: "This agent doesn't have any skills installed yet. Browse the registry to find and install new ones.",
    backToLibrary: "Back to Library",
    updateSkill: "Update Skill",
    verifiedSkill: "Verified Skill",
    terminalVersions: "Terminal Versions",
    sameVersionAcrossTerminals: "same version across terminals",
    multipleTerminalVersionsDetected: "multiple terminal versions detected",
    installation: "Installation",
    coverToAgent: "Cover To This Terminal",
    coverToAllAvailable: "Cover To All Available",
    coveringToAll: "Covering...",
    coveringToAgent: "Covering...",
    coverHistory: "History Rollback",
    rollbackToThisVersion: "Rollback To This Version",
    loadingHistory: "Loading history...",
    noCoverHistory: "No cover history yet",
    coverOperationSummary: (applied: number, skipped: number) => `Cover completed: ${applied} applied, ${skipped} skipped`,
    rollbackSuccess: "Rollback completed",
    inheritedVersion: "Inherited version",
    remove: "Remove",
    install: "Install",
    enabled: "Enabled",
    skillId: "Skill ID",
    lastScanned: "Last scanned",
    minutesAgo: (minutes: number) => `${minutes} mins ago`,
    loadingRegistryFallback: "Failed to load registry",
    searchFailedFallback: "Search failed",
    operationFailedPrefix: "Operation failed",
    agentsCount: (count: number) => `${count} AGENTS`,
    language: "Language",
    chinese: "cn",
    english: "en",
    translatePromptToChinese: "Translate to Chinese",
    translatePromptToEnglish: "Translate to English",
    translating: "Translating...",
    translatedPrompt: "Chinese Prompt",
    translatedPromptEnglish: "English Prompt",
    machineTranslationDisclaimer: "Machine translation, for reference only.",
    translationFailedPrefix: "Translation failed",
    translatorSettingsTitle: "SiliconFlow Translator Settings",
    translatorSettingsDescription: "Use LLM translation for prompts and save config locally.",
    translatorKeyLabel: "SiliconFlow API Key",
    translatorModelLabel: "Model",
    translatorBaseUrlLabel: "Base URL (fixed)",
    saveTranslatorConfig: "Save Translator Settings",
    saving: "Saving...",
    translatorConfigSaved: "Translator settings saved",
    translatorConfigSaveFailed: "Failed to save translator settings",
    testTranslatorConnection: "Connection Test",
    testingConnection: "Testing...",
    translatorConnectionSuccess: "Connection success",
    translatorConnectionFailed: "Connection failed",
    openTranslatorGuide: "Open Setup Guide",
    missingTranslatorConfigPrompt: "SiliconFlow API Key is not configured yet. Go to Settings now?",
    openGuidePrompt: "Open SiliconFlow official guide as well?",
    translatorNoteTitle: "Note",
    translatorNoteBody: "Prompt translation uses SiliconFlow. Base URL is fixed to https://api.siliconflow.cn/v1, with thinking disabled by default.",
    liveTranslationTitle: "Live Translation",
    liveTranslationHint: "Receiving streamed output...",
    liveTranslationRunning: "Translating",
    liveTranslationDone: "Completed",
    liveTranslationError: "Interrupted",
    closeModal: "Close",
    translatorLogsTitle: "Translator Logs",
    translatorLogPath: "Log file",
    refreshLogs: "Refresh Logs",
    openLogFile: "Open Log File",
    openDirectory: "Open Directory",
    loadingLogs: "Loading logs...",
    noLogsYet: "No logs yet. Run a connection test or translation first.",
    claudeInitBannerTitle: "Initialize Claude Skills first",
    claudeInitBannerExistingBody: "Claude's skills directory is already present. Install the recommended starter skills and make them visible in SkillDeck immediately.",
    claudeInitBannerMissingBody: "Claude's skills directory was not found yet. Create ~/.claude/skills first, then download the recommended starter skills locally.",
    claudeInitBannerAction: "Initialize Claude Skills",
    claudeInitBannerLater: "Later",
    claudeInitBannerDocs: "View official guide",
    claudeInitModalTitle: "Claude Skills Bootstrap",
    claudeInitModalSubtitle: "Local-directory-first setup: install Anthropic's official skills into ~/.claude/skills so SkillDeck can scan them right away.",
    claudeInitTargetDir: "Target directory",
    claudeInitSourceRepo: "Source repository",
    claudeInitRecommended: "Recommended starter skills",
    claudeInitOptional: "Optional document skills",
    claudeInitSelectionHint: "Choose the skills to install",
    claudeInitCreateAndInstall: "Create directory and install",
    claudeInitInstall: "Install to Claude Skills",
    claudeInitInstallRunning: "Installing...",
    claudeInitClose: "Close",
    claudeInitOpenDocs: "Open Claude guide",
    claudeInitLoading: "Loading Claude bootstrap info...",
    claudeInitCliMissingNotice: "Claude CLI was not detected. Local installation still works, but you should install Claude Code afterwards.",
    claudeInitTargetMissingNotice: "~/.claude/skills was not found. It will be created automatically during installation.",
    claudeInitTargetReadyNotice: "~/.claude/skills is already available. The recommended bundle can be installed directly.",
    claudeInitCannotCreateNotice: "SkillDeck could not confirm that ~/.claude/skills is creatable. Check home-directory permissions first.",
    claudeInitExistingSkillsNotice: (count: number) => `${count} skills with the same slug already exist in the target directory and will be skipped.`,
    claudeInitExistingSkillTag: "Exists",
    claudeInitNoSkillsSelected: "Select at least one skill",
    claudeInitInstalledSummary: (installed: number, skipped: number) => `Bootstrap finished: ${installed} installed, ${skipped} skipped`,
    claudeInitInstalledList: "Installed",
    claudeInitSkippedList: "Skipped",
    agentStatusCli: "CLI Ready",
    agentStatusCliDescription: "CLI entrypoint detected and ready to use.",
    agentStatusSkills: "Skills Ready",
    agentStatusSkillsDescription: "No CLI found, but a skills directory or local skills were detected.",
    agentStatusConfig: "Config Found",
    agentStatusConfigDescription: "Config directory detected and ready for skill setup.",
    agentStatusMissing: "Not Detected",
    agentStatusMissingDescription: "No CLI, config directory, or skills directory detected.",
    statusSkillCount: (count: number) => `${count} local skills`,
    cursorSkillReadNoteTitle: "Cursor Compatible Read Paths",
    cursorSkillReadNoteBody: "Besides Cursor's own skills directory, Cursor also reads skills from these compatibility paths.",
    sourceAgentLabel: "Source",
    pathExists: "Found",
    pathMissing: "Missing",
    sameVersion: "same version",
    specificVersion: "specific version",
    activeVersionPrefix: "active",
    skillDivergedTag: "diverged",
  },
} as const;

export const getMessages = (locale: Locale) => messages[locale];

export const getInitialLocale = (): Locale => {
  const stored = window.localStorage.getItem(LOCALE_STORAGE_KEY);
  return stored === "en" ? "en" : DEFAULT_LOCALE;
};

export const saveLocale = (locale: Locale) => {
  window.localStorage.setItem(LOCALE_STORAGE_KEY, locale);
};

export const localizeErrorMessage = (rawError: unknown, locale: Locale): string => {
  const source = String(rawError ?? "");
  const lower = source.toLowerCase();

  if (lower.includes("failed to load registry")) {
    return getMessages(locale).failedToLoadRegistry;
  }
  if (lower.includes("search failed")) {
    return getMessages(locale).searchFailedFallback;
  }
  if (lower.includes("skill not found")) {
    return locale === "zh" ? "未找到技能" : "Skill not found";
  }
  if (lower.includes("could not determine agent skills directory")) {
    return locale === "zh" ? "无法确定代理技能目录" : "Could not determine agent skills directory";
  }
  if (lower.includes("skill uid not found")) {
    return locale === "zh" ? "未找到技能版本" : "Skill version not found";
  }
  if (lower.includes("cover history entry not found")) {
    return locale === "zh" ? "未找到覆盖历史记录" : "Cover history entry not found";
  }
  if (lower.includes("rollback backup missing")) {
    return locale === "zh" ? "回退备份缺失，无法恢复" : "Rollback backup is missing";
  }
  if (lower.includes("translator_config_missing")) {
    return locale === "zh"
      ? "未配置硅基流动翻译，请先到设置页填写 SiliconFlow API Key"
      : "SiliconFlow translator is not configured. Please fill SiliconFlow API Key in Settings.";
  }
  if (lower.includes("translator_empty_text")) {
    return locale === "zh" ? "没有可翻译的内容" : "No text to translate";
  }
  if (lower.includes("translator_http_")) {
    return locale === "zh" ? "翻译服务请求失败，请检查 API Key、模型和配额" : "Translator request failed. Check API key, model, and quota.";
  }
  if (lower.includes("translator_request_failed")) {
    return locale === "zh" ? "无法连接翻译服务" : "Could not reach translator service";
  }
  if (lower.includes("translator_stream_failed")) {
    return locale === "zh" ? "流式连接中断，请重试" : "Streaming connection interrupted. Please retry.";
  }
  if (lower.includes("translator_parse_failed") || lower.includes("translator_invalid_response")) {
    return locale === "zh" ? "翻译服务返回数据异常" : "Translator returned an invalid response";
  }
  if (lower.includes("failed to read translator config") || lower.includes("failed to parse translator config")) {
    return locale === "zh" ? "读取翻译配置失败，请重新保存一次" : "Failed to load translator settings. Please save them again.";
  }
  if (lower.includes("failed to write translator config") || lower.includes("failed to create config directory")) {
    return locale === "zh" ? "写入翻译配置失败，请检查本地目录权限" : "Failed to persist translator settings. Check local directory permissions.";
  }
  if (lower.includes("failed to read translator log")) {
    return locale === "zh" ? "读取日志失败，请检查日志文件权限" : "Failed to read translator log. Check file permissions.";
  }
  if (lower.includes("claude_bootstrap_no_skills_selected")) {
    return locale === "zh" ? "请至少选择一个 Claude skill" : "Select at least one Claude skill";
  }
  if (lower.includes("claude_bootstrap_invalid_skill_slug")) {
    return locale === "zh" ? "选择的 Claude skill 无效" : "The selected Claude skill is invalid";
  }
  if (lower.includes("claude_bootstrap_target_dir_missing")) {
    return locale === "zh" ? "Claude 技能目录不存在，请允许应用先创建目录" : "Claude's skills directory is missing. Allow SkillDeck to create it first.";
  }
  if (lower.includes("claude_bootstrap_target_dir_not_creatable")) {
    return locale === "zh" ? "无法创建 Claude 技能目录，请检查家目录权限" : "Claude's skills directory cannot be created. Check home-directory permissions.";
  }
  if (lower.includes("claude_bootstrap_target_dir_create_failed")) {
    return locale === "zh" ? "创建 Claude 技能目录失败，请检查目录权限" : "Failed to create Claude's skills directory. Check directory permissions.";
  }
  if (lower.includes("claude_bootstrap_source_skill_missing")) {
    return locale === "zh" ? "下载源仓库中缺少所选 skill" : "The selected skill is missing from the source repository";
  }
  if (lower.includes("claude_bootstrap_clone_failed")) {
    return locale === "zh" ? "拉取 anthropics/skills 失败，请检查网络连接" : "Failed to clone anthropics/skills. Check your network connection.";
  }

  return source;
};
