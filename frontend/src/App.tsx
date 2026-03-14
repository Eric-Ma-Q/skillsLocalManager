import React, { useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-shell";
import { Wrench, Activity, Grid2X2, Globe, Settings, Search, Download, Star } from "lucide-react";
import { SkillCard } from "./components/SkillCard";
import { ClaudeInitBanner } from "./components/ClaudeInitBanner";
import { SkillDetail } from "./components/SkillDetail";
import { RegistrySkillDetailPanel } from "./components/RegistrySkillDetail";
import { TargetPickerModal } from "./components/TargetPickerModal";
import { formatAgentName, getAgentStatusMeta, isAgentDetected, type Agent } from "./agentStatus";
import { clsx, type ClassValue } from "clsx";
import { twMerge } from "tailwind-merge";
import { getInitialLocale, getMessages, localizeErrorMessage, saveLocale, type Locale } from "./i18n";
import type {
  LocalSortMode,
  ManagedSkillActionResponse,
  RegistrySkill,
  RegistrySkillDetail,
  RegistrySkillsResponse,
  RegistrySortMode,
  SkillGroup,
  SkillVariant,
  TargetMode,
} from "./types";

function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

interface TranslatorConfig {
  source: "siliconflow" | "openrouter";
  siliconflowApiKey: string;
  siliconflowModel: string;
  openrouterApiKey: string;
  openrouterModel: string;
}

type TranslatorConfigPayload = Partial<Omit<TranslatorConfig, "source">> & {
  source?: string;
  apiKey?: string;
  model?: string;
};

const SILICONFLOW_DEFAULT_MODEL = "Qwen/Qwen3.5-4B";
const OPENROUTER_DEFAULT_MODEL = "stepfun/step-3.5-flash:free";

const getTranslatorSource = (source?: string): TranslatorConfig["source"] =>
  source === "openrouter" ? "openrouter" : "siliconflow";

const normalizeTranslatorConfig = (raw?: TranslatorConfigPayload): TranslatorConfig => {
  const source = getTranslatorSource(raw?.source);
  const siliconflowApiKey =
    raw?.siliconflowApiKey ?? (source === "siliconflow" ? (raw?.apiKey ?? "") : "");
  const openrouterApiKey =
    raw?.openrouterApiKey ?? (source === "openrouter" ? (raw?.apiKey ?? "") : "");
  const siliconflowModel = raw?.siliconflowModel
    ?? (source === "siliconflow" ? raw?.model : undefined)
    ?? SILICONFLOW_DEFAULT_MODEL;
  const openrouterModel = raw?.openrouterModel
    ?? (source === "openrouter" ? raw?.model : undefined)
    ?? OPENROUTER_DEFAULT_MODEL;

  return {
    source,
    siliconflowApiKey,
    siliconflowModel,
    openrouterApiKey,
    openrouterModel,
  };
}

const managedOperationNotice = (
  response: ManagedSkillActionResponse,
  t: ReturnType<typeof getMessages>
) => {
  const parts: string[] = [];
  if (response.updatedSource) {
    parts.push(t.updateDownloaded(response.sourceVersionLabel));
  } else if (response.alreadyLatest) {
    parts.push(t.updateAlreadyLatest(response.sourceVersionLabel));
  }
  parts.push(t.targetOperationSummary(response.results.length, response.skipped.length));
  return parts.join(" · ");
};

function buildSkillGroups(variants: SkillVariant[]): SkillGroup[] {
  const groups = new Map<string, SkillGroup>();

  for (const variant of variants) {
    const key = variant.slug;
    const existing = groups.get(key);
    if (!existing) {
      groups.set(key, {
        id: variant.slug,
        slug: variant.slug,
        displayName: variant.metadata.name || variant.slug,
        description: variant.metadata.description || "",
        installations: [...variant.installations],
        variants: [variant],
        hasDiverged: variant.conflictState === "diverged",
      });
      continue;
    }

    existing.variants.push(variant);
    if (!existing.description && variant.metadata.description) {
      existing.description = variant.metadata.description;
    }
    if (!existing.displayName && variant.metadata.name) {
      existing.displayName = variant.metadata.name;
    }
    if (variant.conflictState === "diverged") {
      existing.hasDiverged = true;
    }

    for (const inst of variant.installations) {
      if (!existing.installations.some((x) => x.agentType === inst.agentType)) {
        existing.installations.push(inst);
      }
    }
  }

  const result = Array.from(groups.values());
  for (const group of result) {
    const hashes = new Set(group.variants.map((v) => v.treeHash));
    if (hashes.size > 1) {
      group.hasDiverged = true;
    }
    group.variants.sort((a, b) => a.namespace.localeCompare(b.namespace));
  }
  result.sort((a, b) => a.displayName.toLowerCase().localeCompare(b.displayName.toLowerCase()));
  return result;
}

export default function App() {
  const [agents, setAgents] = useState<Agent[]>([]);
  const [variants, setVariants] = useState<SkillVariant[]>([]);
  const [selectedAgent, setSelectedAgent] = useState<string | null>(null);
  const [selectedSkill, setSelectedSkill] = useState<SkillGroup | null>(null);
  const [loading, setLoading] = useState(true);
  const [view, setView] = useState<"skills" | "registry" | "settings">("skills");
  const [registrySkills, setRegistrySkills] = useState<RegistrySkill[]>([]);
  const [registryLoading, setRegistryLoading] = useState(false);
  const [registryLoadingMore, setRegistryLoadingMore] = useState(false);
  const [registrySearch, setRegistrySearch] = useState("");
  const [registrySort, setRegistrySort] = useState<RegistrySortMode>("updated");
  const [registryNextCursor, setRegistryNextCursor] = useState<string | null>(null);
  const [registryError, setRegistryError] = useState<string | null>(null);
  const [registryDetail, setRegistryDetail] = useState<RegistrySkillDetail | null>(null);
  const [registryTransitioningToLocal, setRegistryTransitioningToLocal] = useState(false);
  const [registryDetailLoadingSlug, setRegistryDetailLoadingSlug] = useState<string | null>(null);
  const [registryInstallingSlug, setRegistryInstallingSlug] = useState<string | null>(null);
  const [skillsSearch, setSkillsSearch] = useState("");
  const [skillsSort, setSkillsSort] = useState<LocalSortMode>("name");
  const [translatorConfig, setTranslatorConfig] = useState<TranslatorConfig>(() =>
    normalizeTranslatorConfig()
  );
  const [translatorSaving, setTranslatorSaving] = useState(false);
  const [translatorTesting, setTranslatorTesting] = useState(false);
  const [translatorNotice, setTranslatorNotice] = useState<string | null>(null);
  const [translatorLogPath, setTranslatorLogPath] = useState("");
  const [translatorLogs, setTranslatorLogs] = useState("");
  const [translatorLogsLoading, setTranslatorLogsLoading] = useState(false);
  const [locale, setLocale] = useState<Locale>(() => getInitialLocale());
  const [targetPicker, setTargetPicker] = useState<null | {
    title: string;
    description: string;
    confirmLabel: string;
    onConfirm: (selection: { targetMode: TargetMode; targetAgentType?: string }) => Promise<void>;
  }>(null);
  const t = getMessages(locale);
  const skills = useMemo(() => buildSkillGroups(variants), [variants]);
  const agentAccessibleSkillCounts = useMemo(() => {
    const counts = new Map<string, number>();
    for (const agent of agents) {
      counts.set(agent.agentType, 0);
    }

    for (const skill of skills) {
      const seenAgentTypes = new Set<string>();
      for (const installation of skill.installations) {
        const agentType = String(installation.agentType);
        if (seenAgentTypes.has(agentType)) {
          continue;
        }
        seenAgentTypes.add(agentType);
        counts.set(agentType, (counts.get(agentType) ?? 0) + 1);
      }
    }
    return counts;
  }, [agents, skills]);

  const getAgentDisplaySkillCount = (agent: Agent) => {
    const accessible = agentAccessibleSkillCounts.get(agent.agentType);
    if (accessible === undefined) {
      return agent.skillCount;
    }
    return Math.max(agent.skillCount, accessible);
  };

  const refreshData = async () => {
    const [agentsRes, skillsRes] = await Promise.all([
      invoke("get_agents"),
      invoke("get_skills_v2")
    ]) as [any, any];
    setAgents(agentsRes);
    const nextVariants = skillsRes as SkillVariant[];
    setVariants(nextVariants);
    setLoading(false);

    if (selectedSkill) {
      const updated = buildSkillGroups(nextVariants).find(s => s.slug === selectedSkill.slug);
      if (updated) setSelectedSkill(updated);
    }
  };

  useEffect(() => {
    saveLocale(locale);
    document.documentElement.lang = locale === "zh" ? "zh-CN" : "en";
    document.title = getMessages(locale).appName;
  }, [locale]);

  useEffect(() => {
    refreshData();
  }, []);

  useEffect(() => {
    invoke<TranslatorConfigPayload>("get_translator_config")
      .then((config) => setTranslatorConfig(normalizeTranslatorConfig(config)))
      .catch(() => {
        // Keep default empty config.
      });
  }, []);

  const loadTranslatorLogs = async () => {
    setTranslatorLogsLoading(true);
    try {
      const [path, tail] = await Promise.all([
        invoke<string>("get_translator_log_path"),
        invoke<string>("get_translator_log_tail", { maxLines: 180 }),
      ]);
      setTranslatorLogPath(path);
      setTranslatorLogs(tail);
    } catch (err) {
      setTranslatorNotice(localizeErrorMessage(err, locale));
    } finally {
      setTranslatorLogsLoading(false);
    }
  };

  useEffect(() => {
    if (view === "settings") {
      loadTranslatorLogs();
    }
  }, [view]);

  const loadRegistry = async ({
    append = false,
    cursor = null,
    query = registrySearch,
    sort = registrySort,
  }: {
    append?: boolean;
    cursor?: string | null;
    query?: string;
    sort?: RegistrySortMode;
  } = {}) => {
    if (append) {
      setRegistryLoadingMore(true);
    } else {
      setRegistryLoading(true);
    }
    setRegistryError(null);
    try {
      const res = await invoke<RegistrySkillsResponse>("get_registry_skills", {
        request: {
          query: query.trim() || null,
          sort,
          cursor,
          limit: 24,
        },
      });
      setRegistrySkills((previous) => (append ? [...previous, ...res.items] : res.items));
      setRegistryNextCursor(res.nextCursor ?? null);
    } catch (err) {
      setRegistryError(localizeErrorMessage(err, locale));
    } finally {
      if (append) {
        setRegistryLoadingMore(false);
      } else {
        setRegistryLoading(false);
      }
    }
  };

  const openRegistryDetail = async (slug: string) => {
    setRegistryDetailLoadingSlug(slug);
    try {
      const detail = await invoke<RegistrySkillDetail>("get_registry_skill_detail", { slug });
      setRegistryDetail(detail);
    } catch (err) {
      alert(`${t.operationFailedPrefix}: ${localizeErrorMessage(err, locale)}`);
    } finally {
      setRegistryDetailLoadingSlug(null);
    }
  };

  const openInstallTargetPicker = (skill: RegistrySkill | RegistrySkillDetail) => {
    setTargetPicker({
      title: t.installSkillToTargets,
      description: `${skill.displayName} · ${skill.latestVersion ? `v${skill.latestVersion}` : t.latestVersionUnknown}`,
      confirmLabel: t.install,
      onConfirm: async ({ targetMode, targetAgentType }) => {
        setRegistryInstallingSlug(skill.slug);
        try {
          const response = await invoke<ManagedSkillActionResponse>("install_registry_skill", {
            request: {
              slug: skill.slug,
              versionOrTag: skill.latestVersion ?? null,
              targetMode,
              targetAgentType: targetAgentType ?? null,
            },
          });
          alert(managedOperationNotice(response, t));
          await refreshData();
        } catch (err) {
          alert(`${t.operationFailedPrefix}: ${localizeErrorMessage(err, locale)}`);
        } finally {
          setRegistryInstallingSlug(null);
        }
      },
    });
  };

  const saveTranslatorConfig = async () => {
    setTranslatorSaving(true);
    setTranslatorNotice(null);
    try {
      await invoke("save_translator_config", { config: translatorConfig });
      setTranslatorNotice(t.translatorConfigSaved);
    } catch (err) {
      setTranslatorNotice(`${t.translatorConfigSaveFailed}: ${localizeErrorMessage(err, locale)}`);
    } finally {
      setTranslatorSaving(false);
    }
  };

  const testTranslatorConnection = async () => {
    setTranslatorTesting(true);
    setTranslatorNotice(null);
    try {
      await invoke("save_translator_config", { config: translatorConfig });
      const result = await invoke<string>("test_translator_connection");
      setTranslatorNotice(`${t.translatorConnectionSuccess}: ${result}`);
      await loadTranslatorLogs();
    } catch (err) {
      setTranslatorNotice(`${t.translatorConnectionFailed}: ${localizeErrorMessage(err, locale)}`);
      await loadTranslatorLogs();
    } finally {
      setTranslatorTesting(false);
    }
  };

  const activeTranslatorApiKey = translatorConfig.source === "openrouter"
    ? translatorConfig.openrouterApiKey
    : translatorConfig.siliconflowApiKey;
  const activeTranslatorModel = translatorConfig.source === "openrouter"
    ? translatorConfig.openrouterModel
    : translatorConfig.siliconflowModel;
  const activeTranslatorNoteBody = translatorConfig.source === "openrouter"
    ? t.translatorNoteBodyOpenRouter
    : t.translatorNoteBodySiliconFlow;

  const openTranslatorGuide = async () => {
    const guideUrl = translatorConfig.source === "openrouter"
      ? "https://openrouter.ai/docs/quickstart"
      : "https://cloud.siliconflow.cn/i/wRp8aT8o";
    try {
      await open(guideUrl);
    } catch {
      window.open(guideUrl, "_blank");
    }
  };

  const openDirectory = async (path: string, event: React.MouseEvent) => {
    event.stopPropagation();
    event.preventDefault();
    try {
      await invoke("open_local_path", { path });
    } catch {
      // Ignore local-open failures in the sidebar action.
    }
  };

  const openTranslatorLogFile = async () => {
    if (!translatorLogPath) {
      return;
    }
    try {
      await invoke("open_local_path", { path: translatorLogPath });
    } catch (err) {
      setTranslatorNotice(`${t.operationFailedPrefix}: ${localizeErrorMessage(err, locale)}`);
    }
  };

  const normalizedSkillsSearch = skillsSearch.trim().toLowerCase();
  const filteredSkills = skills
    .filter((skill) => (selectedAgent ? skill.installations.some((i) => i.agentType === selectedAgent) : true))
    .filter((skill) => {
      if (!normalizedSkillsSearch) return true;
      const haystacks = [
        skill.displayName,
        skill.slug,
        skill.description,
        ...skill.variants.map((variant) => variant.metadata.description || ""),
      ];
      return haystacks.some((value) => value.toLowerCase().includes(normalizedSkillsSearch));
    })
    .sort((left, right) => {
      if (skillsSort === "modified") {
        const leftModified = Math.max(...left.variants.map((variant) => variant.modifiedAt ?? 0), 0);
        const rightModified = Math.max(...right.variants.map((variant) => variant.modifiedAt ?? 0), 0);
        return rightModified - leftModified || left.displayName.localeCompare(right.displayName);
      }
      return left.displayName.toLowerCase().localeCompare(right.displayName.toLowerCase());
    });
  const pageTitle = view === "registry"
    ? t.browseRegistry
    : view === "settings"
      ? t.settings
      : selectedAgent
        ? formatAgentName(selectedAgent)
        : t.allSkills;
  const showClaudeInitBanner = !loading && variants.length === 0;

  const openLocalSkillFromRegistry = (slug: string) => {
    const localSkill = skills.find((skill) => skill.slug === slug);
    if (!localSkill) return;
    setView("skills");
    setSelectedAgent(null);
    setSelectedSkill(localSkill);
    setRegistryTransitioningToLocal(true);
    window.setTimeout(() => {
      setRegistryDetail(null);
      setRegistryTransitioningToLocal(false);
    }, 180);
  };

  return (
    <div className="flex h-screen bg-[#FDFDFF] overflow-hidden">
      {/* Sidebar */}
      <div className="w-72 bg-[#F1F1F6]/80 backdrop-blur-xl border-r border-slate-200/50 flex flex-col">
        <div className="p-6 flex items-center space-x-3">
          <div className="bg-indigo-600 p-2 rounded-xl">
            <Wrench className="w-6 h-6 text-white" />
          </div>
          <div>
            <h1 className="text-lg font-bold text-slate-900 leading-tight">{t.appName}</h1>
            <p className="text-[11px] font-bold text-slate-400 tracking-widest uppercase">{t.appSubtitle}</p>
          </div>
        </div>

        <nav className="flex-1 px-4 overflow-y-auto">
          <div className="mb-8">
            <h2 className="px-3 mb-2 text-[10px] font-bold text-slate-400 uppercase tracking-widest">{t.general}</h2>
            <button 
              onClick={() => { setSelectedAgent(null); setView("skills"); }}
              className={cn(
                "w-full flex items-center justify-between px-3 py-2.5 rounded-xl text-sm transition-all duration-200 group",
                selectedAgent === null && view === "skills" ? "bg-white shadow-sm text-indigo-600 font-semibold" : "text-slate-500 hover:bg-slate-200/50"
              )}
            >
              <div className="flex items-center space-x-3">
                <Grid2X2 className="w-4 h-4" />
                <span>{t.allSkills}</span>
              </div>
              <span className="text-[10px] bg-slate-200 text-slate-600 px-1.5 py-0.5 rounded-full">{skills.length}</span>
            </button>
            <button
              onClick={() => {
                setView("registry");
                setSelectedAgent(null);
                setRegistryDetail(null);
                loadRegistry({ query: registrySearch, sort: registrySort });
              }}
              className={cn(
                "w-full mt-1 flex items-center justify-between px-3 py-2.5 rounded-xl text-sm transition-all duration-200",
                view === "registry" ? "bg-white shadow-sm text-indigo-600 font-semibold" : "text-slate-500 hover:bg-slate-200/50"
              )}
            >
              <div className="flex items-center space-x-3">
                <Globe className="w-4 h-4" />
                <span>{t.browseRegistry}</span>
              </div>
            </button>
          </div>

          <div>
            <h2 className="px-3 mb-2 text-[10px] font-bold text-slate-400 uppercase tracking-widest">{t.installedAgents}</h2>
            <div className="space-y-1">
              {agents.map((agent) => (
                (() => {
                  const displaySkillCount = getAgentDisplaySkillCount(agent);
                  const status = getAgentStatusMeta({ ...agent, skillCount: displaySkillCount }, locale);
                  return (
                    <div
                      key={agent.agentType}
                      className={cn(
                        "rounded-xl transition-all duration-200",
                        selectedAgent === agent.agentType ? "bg-white shadow-sm text-indigo-600 font-semibold" : "text-slate-500 hover:bg-slate-200/50"
                      )}
                    >
                      <button
                        onClick={() => { setSelectedAgent(agent.agentType); setView("skills"); }}
                        className="w-full px-3 py-3 text-sm"
                      >
                        <div className="flex items-center justify-between gap-3 text-left">
                          <div className="flex min-w-0 items-center space-x-3">
                            <div className={cn("h-2.5 w-2.5 shrink-0 rounded-full", status.dotClassName)} />
                            <span className="truncate capitalize text-slate-800">{formatAgentName(agent.agentType)}</span>
                          </div>
                          <span className="rounded-full bg-slate-200 px-1.5 py-0.5 text-[10px] font-medium text-slate-600">
                            {displaySkillCount}
                          </span>
                        </div>
                      </button>
                      <div className="flex items-center justify-between gap-3 pb-3 pl-[25px] pr-3">
                        {agent.skillsDirectory ? (
                          <button
                            onClick={(event) => openDirectory(agent.skillsDirectory!, event)}
                            className="text-[11px] font-medium text-slate-400 transition-colors hover:text-indigo-600"
                          >
                            {t.openDirectory}
                          </button>
                        ) : (
                          <span />
                        )}
                        <span className={cn("rounded-full px-2 py-1 text-[10px] font-semibold", status.badgeClassName)}>
                          {status.label}
                        </span>
                      </div>
                    </div>
                  );
                })()
              ))}
            </div>
          </div>
        </nav>

        <div className="p-4 border-t border-slate-200/50">
          <button
            onClick={() => { setView("settings"); setSelectedAgent(null); }}
            className={cn(
              "w-full flex items-center space-x-3 px-3 py-2.5 rounded-xl text-sm transition-all duration-200",
              view === "settings" ? "bg-white shadow-sm text-indigo-600 font-semibold" : "text-slate-500 hover:bg-slate-200/50"
            )}
          >
            <Settings className="w-4 h-4" />
            <span>{t.settings}</span>
          </button>
        </div>
      </div>

      {/* Main Content */}
      <main className="flex-1 flex flex-col min-w-0">
        <header className="h-16 flex items-center justify-between px-8 border-b border-slate-200/50">
          <div className="flex flex-col">
            <h2 className="text-xl font-bold text-slate-800 capitalize">{pageTitle}</h2>
            {view === "skills" && (
              <p className="text-[11px] text-slate-400">{t.skillDotsHint}</p>
            )}
          </div>
          <div className="flex items-center space-x-2">
            <div className="flex items-center space-x-2 bg-white border border-slate-200 rounded-lg p-1.5">
              <span className="text-xs text-slate-500">{t.language}</span>
              <button
                onClick={() => setLocale("zh")}
                className={cn(
                  "px-2 py-1 text-xs rounded-md transition-colors",
                  locale === "zh" ? "bg-indigo-50 text-indigo-600 font-semibold" : "text-slate-500 hover:text-slate-700"
                )}
              >
                {t.chinese}
              </button>
              <button
                onClick={() => setLocale("en")}
                className={cn(
                  "px-2 py-1 text-xs rounded-md transition-colors",
                  locale === "en" ? "bg-indigo-50 text-indigo-600 font-semibold" : "text-slate-500 hover:text-slate-700"
                )}
              >
                {t.english}
              </button>
            </div>
            {view === "registry" ? (
              <>
                <div className="flex items-center space-x-2 rounded-lg border border-slate-200 bg-white px-3 py-1.5">
                  <Search className="w-3.5 h-3.5 text-slate-400" />
                  <input
                    type="text"
                    placeholder={t.registrySearchPlaceholder}
                    value={registrySearch}
                    onChange={(e) => setRegistrySearch(e.target.value)}
                    onKeyDown={(e) => {
                      if (e.key === "Enter") {
                        loadRegistry({ query: registrySearch, sort: registrySort });
                      }
                    }}
                    className="w-40 bg-transparent text-xs text-slate-600 outline-none placeholder:text-slate-400"
                  />
                </div>
                <select
                  value={registrySort}
                  onChange={(e) => {
                    const nextSort = e.target.value as RegistrySortMode;
                    setRegistrySort(nextSort);
                    loadRegistry({ query: registrySearch, sort: nextSort });
                  }}
                  className="rounded-lg border border-slate-200 bg-white px-3 py-1.5 text-xs text-slate-600 outline-none"
                >
                  <option value="updated">{t.sortUpdated}</option>
                  <option value="downloads">{t.sortDownloads}</option>
                  <option value="name">{t.sortName}</option>
                </select>
              </>
            ) : view === "settings" ? null : (
              <>
                <div className="flex items-center space-x-2 rounded-lg border border-slate-200 bg-white px-3 py-1.5">
                  <Search className="w-3.5 h-3.5 text-slate-400" />
                  <input
                    type="text"
                    placeholder={t.localSearchPlaceholder}
                    value={skillsSearch}
                    onChange={(e) => setSkillsSearch(e.target.value)}
                    className="w-40 bg-transparent text-xs text-slate-600 outline-none placeholder:text-slate-400"
                  />
                </div>
                <select
                  value={skillsSort}
                  onChange={(e) => setSkillsSort(e.target.value as LocalSortMode)}
                  className="rounded-lg border border-slate-200 bg-white px-3 py-1.5 text-xs text-slate-600 outline-none"
                >
                  <option value="name">{t.sortName}</option>
                  <option value="modified">{t.sortModified}</option>
                </select>
                <div className="flex items-center space-x-2 rounded-lg border border-slate-200 bg-white px-3 py-1.5 text-xs font-medium text-slate-600">
                  <Activity className="w-3.5 h-3.5 text-green-500" />
                  <span>{t.totalCount(filteredSkills.length)}</span>
                </div>
              </>
            )}
          </div>
        </header>

        <ClaudeInitBanner enabled={showClaudeInitBanner} locale={locale} onInstalled={refreshData} />

        <div className="flex-1 overflow-y-auto p-8">
          {view === "settings" ? (
            <div className="max-w-2xl space-y-6">
              <div className="bg-white rounded-2xl border border-slate-200/60 p-6">
                <h3 className="font-semibold text-slate-800 text-sm mb-2">{t.translatorSettingsTitle}</h3>
                <p className="text-xs text-slate-500 mb-4">{t.translatorSettingsDescription}</p>
                <div className="mb-4 inline-flex rounded-lg border border-slate-200 bg-slate-50 p-1">
                  <button
                    onClick={() => {
                      setTranslatorConfig((prev) => ({ ...prev, source: "siliconflow" }));
                    }}
                    className={cn(
                      "rounded-md px-3 py-1.5 text-xs font-medium transition-colors",
                      translatorConfig.source === "siliconflow"
                        ? "bg-white text-indigo-700 shadow-sm"
                        : "text-slate-600 hover:text-slate-800"
                    )}
                  >
                    {t.translatorSourceSiliconFlow}
                  </button>
                  <button
                    onClick={() => {
                      setTranslatorConfig((prev) => ({ ...prev, source: "openrouter" }));
                    }}
                    className={cn(
                      "rounded-md px-3 py-1.5 text-xs font-medium transition-colors",
                      translatorConfig.source === "openrouter"
                        ? "bg-white text-indigo-700 shadow-sm"
                        : "text-slate-600 hover:text-slate-800"
                    )}
                  >
                    {t.translatorSourceOpenRouter}
                  </button>
                </div>
                <div className="space-y-3">
                  <div>
                    <label className="text-xs text-slate-500 mb-1 block">{t.translatorKeyLabel}</label>
                    <input
                      type="password"
                      value={activeTranslatorApiKey}
                      onChange={(e) => {
                        const nextApiKey = e.target.value;
                        setTranslatorConfig((prev) => prev.source === "openrouter"
                          ? { ...prev, openrouterApiKey: nextApiKey }
                          : { ...prev, siliconflowApiKey: nextApiKey }
                        );
                      }}
                      className="w-full bg-slate-50 border border-slate-200 rounded-lg px-3 py-2 text-xs text-slate-700 outline-none focus:border-indigo-300 focus:ring-2 focus:ring-indigo-100"
                    />
                  </div>
                  <p className="text-[11px] text-slate-500">
                    {t.translatorDefaultModelLabel}:{" "}
                    <span className="font-mono text-slate-700">{activeTranslatorModel}</span>
                  </p>
                </div>
                <div className="mt-4 flex items-center gap-2">
                  <button
                    onClick={saveTranslatorConfig}
                    disabled={translatorSaving}
                    className="px-4 py-2 bg-indigo-600 text-white text-xs rounded-lg hover:bg-indigo-700 transition-colors disabled:opacity-70 disabled:cursor-wait"
                  >
                    {translatorSaving ? t.saving : t.saveTranslatorConfig}
                  </button>
                  <button
                    onClick={testTranslatorConnection}
                    disabled={translatorTesting}
                    className="px-4 py-2 bg-emerald-600 text-white text-xs rounded-lg hover:bg-emerald-700 transition-colors disabled:opacity-70 disabled:cursor-wait"
                  >
                    {translatorTesting ? t.testingConnection : t.testTranslatorConnection}
                  </button>
                </div>
                {translatorNotice && (
                  <p className="mt-3 text-xs text-slate-500">{translatorNotice}</p>
                )}
                <div className="mt-4 rounded-lg border border-blue-100 bg-blue-50/70 p-3">
                  <p className="text-xs font-semibold text-blue-800">{t.translatorNoteTitle}</p>
                  <p className="mt-1 text-xs text-blue-700">{activeTranslatorNoteBody}</p>
                  <button
                    onClick={openTranslatorGuide}
                    className="mt-2 text-xs text-blue-700 underline hover:text-blue-800"
                  >
                    {t.openTranslatorGuide}
                  </button>
                </div>
              </div>

              <div className="bg-white rounded-2xl border border-slate-200/60 p-6">
                <div className="flex items-center justify-between">
                  <h3 className="font-semibold text-slate-800 text-sm">{t.translatorLogsTitle}</h3>
                  <div className="flex items-center gap-2">
                    <button
                      onClick={loadTranslatorLogs}
                      disabled={translatorLogsLoading}
                      className="px-3 py-1.5 bg-slate-100 text-slate-700 text-xs rounded-lg hover:bg-slate-200 transition-colors disabled:opacity-70 disabled:cursor-wait"
                    >
                      {translatorLogsLoading ? t.loadingLogs : t.refreshLogs}
                    </button>
                    <button
                      onClick={() => {
                        void openTranslatorLogFile();
                      }}
                      disabled={!translatorLogPath}
                      className="px-3 py-1.5 bg-blue-50 text-blue-700 text-xs rounded-lg hover:bg-blue-100 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
                    >
                      {t.openLogFile}
                    </button>
                  </div>
                </div>
                <div className="mt-3">
                  <p className="text-[11px] text-slate-500">{t.translatorLogPath}</p>
                  <p className="mt-1 text-[11px] text-slate-700 font-mono break-all">
                    {translatorLogPath || "-"}
                  </p>
                </div>
                <div className="mt-3 max-h-72 overflow-y-auto rounded-lg bg-slate-950 p-3 font-mono text-[11px] leading-5 text-slate-100 whitespace-pre-wrap">
                  {translatorLogsLoading
                    ? t.loadingLogs
                    : (translatorLogs.trim() ? translatorLogs : t.noLogsYet)}
                </div>
              </div>

              <div className="bg-white rounded-2xl border border-slate-200/60 p-6">
                <h3 className="font-semibold text-slate-800 text-sm mb-4">{t.skillsDirectory}</h3>
                <div className="space-y-3">
                  <div>
                    <label className="text-xs text-slate-500 mb-1 block">{t.globalSharedSkills}</label>
                    <div className="bg-slate-50 px-3 py-2 rounded-lg text-xs text-slate-600 font-mono">~/.agents/skills/</div>
                  </div>
                  {agents.filter(isAgentDetected).map(agent => {
                    const displaySkillCount = getAgentDisplaySkillCount(agent);
                    const status = getAgentStatusMeta({ ...agent, skillCount: displaySkillCount }, locale);
                    return (
                    <div key={agent.agentType}>
                      <div className="mb-1 flex items-center gap-2">
                        <label className="block text-xs capitalize text-slate-500">{formatAgentName(agent.agentType)}</label>
                        <span className={cn("rounded-full px-2 py-0.5 text-[10px] font-semibold", status.badgeClassName)}>
                          {status.label}
                        </span>
                      </div>
                      {agent.configDirectory && (
                        <div className="mb-2">
                          <label className="mb-1 block text-[11px] text-slate-400">{t.configDirectory}</label>
                          <div className="rounded-lg bg-slate-50 px-3 py-2 font-mono text-xs text-slate-600">{agent.configDirectory}</div>
                        </div>
                      )}
                      {agent.skillsDirectory && (
                        <div>
                          <label className="mb-1 block text-[11px] text-slate-400">{t.skillsDirectory}</label>
                          <div className="rounded-lg bg-slate-50 px-3 py-2 font-mono text-xs text-slate-600">{agent.skillsDirectory}</div>
                        </div>
                      )}
                      {agent.agentType === "cursor" && (agent.readableSkillsDirectories?.length ?? 0) > 0 && (
                        <div className="mt-2 rounded-lg border border-indigo-100 bg-indigo-50/50 p-3">
                          <p className="text-xs font-semibold text-indigo-900">{t.cursorSkillReadNoteTitle}</p>
                          <p className="mt-1 text-[11px] text-indigo-700">{t.cursorSkillReadNoteBody}</p>
                          <div className="mt-2 space-y-2">
                            {(agent.readableSkillsDirectories ?? []).map((entry) => (
                              <div key={`${entry.sourceAgentType}:${entry.path}`} className="rounded-md bg-white/70 px-2.5 py-2">
                                <div className="mb-1 flex items-center justify-between gap-2">
                                  <span className="text-[10px] font-semibold uppercase tracking-wide text-slate-500">
                                    {t.sourceAgentLabel}: {formatAgentName(entry.sourceAgentType)}
                                  </span>
                                  <span
                                    className={cn(
                                      "rounded-full px-1.5 py-0.5 text-[10px] font-semibold",
                                      entry.exists
                                        ? "bg-emerald-100 text-emerald-700"
                                        : "bg-slate-200 text-slate-600"
                                    )}
                                  >
                                    {entry.exists ? t.pathExists : t.pathMissing}
                                  </span>
                                </div>
                                <div className="font-mono text-[11px] text-slate-700 break-all">{entry.path}</div>
                              </div>
                            ))}
                          </div>
                        </div>
                      )}
                    </div>
                    );
                  })}
                </div>
              </div>

              <div className="bg-white rounded-2xl border border-slate-200/60 p-6">
                <h3 className="font-semibold text-slate-800 text-sm mb-4">{t.about}</h3>
                <div className="space-y-2 text-xs text-slate-500">
                  <div className="flex justify-between"><span>{t.version}</span><span className="text-slate-700">0.1.0</span></div>
                  <div className="flex justify-between"><span>{t.platform}</span><span className="text-slate-700">Tauri + React</span></div>
                  <div className="flex justify-between"><span>{t.registry}</span><span className="text-slate-700">clawhub.ai</span></div>
                </div>
              </div>
            </div>
          ) : view === "registry" ? (
            registryLoading ? (
              <div className="flex items-center justify-center h-full">
                <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-indigo-600"></div>
              </div>
            ) : registryError ? (
              <div className="flex flex-col items-center justify-center h-full text-center py-20">
                <div className="bg-red-50 p-6 rounded-full mb-4">
                  <Globe className="w-12 h-12 text-red-400" />
                </div>
                <h3 className="text-lg font-semibold text-slate-800">{t.failedToLoadRegistry}</h3>
                <p className="text-slate-500 max-w-sm mt-2">{localizeErrorMessage(registryError, locale)}</p>
                <button
                  onClick={() => loadRegistry({ query: registrySearch, sort: registrySort })}
                  className="mt-4 rounded-lg bg-indigo-600 px-4 py-2 text-sm text-white transition-colors hover:bg-indigo-700"
                >
                  {t.retry}
                </button>
              </div>
            ) : registrySkills.length > 0 ? (
              <div>
                <div className="grid grid-cols-1 gap-6 md:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4">
                  {registrySkills.map((rs) => (
                    <div
                      key={rs.slug}
                      onClick={() => openRegistryDetail(rs.slug)}
                      className="cursor-pointer rounded-2xl border border-slate-200/60 bg-white p-5 transition-all hover:border-indigo-200 hover:shadow-md"
                    >
                      <div className="mb-3 flex items-start justify-between gap-3">
                        <div>
                          <h3 className="text-sm font-semibold text-slate-800">{rs.displayName}</h3>
                          <p className="mt-1 line-clamp-2 text-xs text-slate-500">{rs.summary}</p>
                        </div>
                        {registryDetailLoadingSlug === rs.slug && (
                          <span className="text-[10px] font-semibold text-indigo-500">{t.loading}</span>
                        )}
                      </div>
                      <div className="flex items-center justify-between text-[10px] text-slate-400">
                        <div className="flex items-center space-x-3">
                          <span className="flex items-center space-x-1"><Download className="w-3 h-3" /><span>{rs.downloads}</span></span>
                          <span className="flex items-center space-x-1"><Star className="w-3 h-3" /><span>{rs.stars}</span></span>
                        </div>
                        {rs.latestVersion && <span className="rounded bg-slate-100 px-1.5 py-0.5 text-slate-500">v{rs.latestVersion}</span>}
                      </div>
                      <div className="mt-4 flex justify-end">
                        <button
                          onClick={(event) => {
                            event.stopPropagation();
                            openInstallTargetPicker(rs);
                          }}
                          disabled={registryInstallingSlug === rs.slug}
                          className="rounded-full bg-indigo-50 px-3 py-1.5 text-[11px] font-semibold text-indigo-700 transition-colors hover:bg-indigo-100 disabled:cursor-not-allowed disabled:opacity-60"
                        >
                          {registryInstallingSlug === rs.slug ? t.working : t.install}
                        </button>
                      </div>
                    </div>
                  ))}
                </div>
                {registryNextCursor && (
                  <div className="mt-6 flex justify-center">
                    <button
                      onClick={() =>
                        loadRegistry({
                          append: true,
                          cursor: registryNextCursor,
                          query: registrySearch,
                          sort: registrySort,
                        })
                      }
                      disabled={registryLoadingMore}
                      className="rounded-full border border-slate-200 bg-white px-4 py-2 text-sm font-medium text-slate-600 transition-colors hover:bg-slate-50 disabled:cursor-not-allowed disabled:opacity-60"
                    >
                      {registryLoadingMore ? t.loadingMore : t.loadMore}
                    </button>
                  </div>
                )}
              </div>
            ) : (
              <div className="flex flex-col items-center justify-center h-full text-center py-20">
                <div className="bg-slate-100 p-6 rounded-full mb-4">
                  <Globe className="w-12 h-12 text-slate-400" />
                </div>
                <h3 className="text-lg font-semibold text-slate-800">{t.noResultsFound}</h3>
                <p className="text-slate-500 max-w-sm mt-2">{t.tryDifferentSearch}</p>
              </div>
            )
          ) : loading ? (
            <div className="flex items-center justify-center h-full">
              <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-indigo-600"></div>
            </div>
          ) : filteredSkills.length > 0 ? (
            <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-6">
              {filteredSkills.map(skill => (
                <div key={skill.id} className="h-full" onClick={() => setSelectedSkill(skill)}>
                  <SkillCard skill={skill} locale={locale} />
                </div>
              ))}
            </div>
          ) : (
            <div className="flex flex-col items-center justify-center h-full text-center py-20">
              <div className="bg-slate-100 p-6 rounded-full mb-4">
                <Grid2X2 className="w-12 h-12 text-slate-400" />
              </div>
              <h3 className="text-lg font-semibold text-slate-800">{t.noSkillsFound}</h3>
              <p className="text-slate-500 max-w-sm mt-2">{t.noSkillsDescription}</p>
            </div>
          )}
        </div>
      </main>

      {/* Skill Detail View */}
      {selectedSkill && (
        <SkillDetail 
          skillGroup={selectedSkill} 
          agents={agents} 
          locale={locale}
          layerClassName={registryDetail ? "z-[70]" : "z-50"}
          onClose={() => setSelectedSkill(null)} 
          onRefresh={refreshData}
          onOpenTranslatorSettings={() => {
            setView("settings");
            setSelectedAgent(null);
          }}
          onOpenTranslatorGuide={openTranslatorGuide}
        />
      )}
      {registryDetail && (
        <RegistrySkillDetailPanel
          agents={agents}
          detail={registryDetail}
          installing={registryInstallingSlug === registryDetail.slug}
          isTransitioningToLocal={registryTransitioningToLocal}
          localSkill={skills.find((skill) => skill.slug === registryDetail.slug) ?? null}
          locale={locale}
          onClose={() => setRegistryDetail(null)}
          onInstall={() => openInstallTargetPicker(registryDetail)}
          onOpenLocalSkill={() => openLocalSkillFromRegistry(registryDetail.slug)}
          onOpenTranslatorSettings={() => {
            setView("settings");
            setSelectedAgent(null);
          }}
          onOpenTranslatorGuide={openTranslatorGuide}
        />
      )}
      {targetPicker && (
        <TargetPickerModal
          agents={agents}
          confirmLabel={targetPicker.confirmLabel}
          description={targetPicker.description}
          locale={locale}
          onClose={() => setTargetPicker(null)}
          onConfirm={targetPicker.onConfirm}
          title={targetPicker.title}
        />
      )}
    </div>
  );
}

