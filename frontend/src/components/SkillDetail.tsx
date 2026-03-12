import React, { useEffect, useMemo, useRef, useState } from "react";
import ReactMarkdown from "react-markdown";
import type { Components } from "react-markdown";
import remarkGfm from "remark-gfm";
import { X, Check, ArrowLeft, Download, ShieldCheck, Info, Languages, History } from "lucide-react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { clsx, type ClassValue } from "clsx";
import { twMerge } from "tailwind-merge";
import { formatAgentName, getAgentStatusMeta, type Agent } from "../agentStatus";
import { getMessages, localizeErrorMessage, type Locale } from "../i18n";

function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

interface SkillVariant {
  id: string;
  uid: string;
  slug: string;
  namespace: string;
  treeHash: string;
  metadata: { name: string; description: string; version?: string };
  markdownBody: string;
  installations: any[];
}

interface SkillGroup {
  id: string;
  slug: string;
  displayName: string;
  description: string;
  installations: any[];
  variants: SkillVariant[];
  hasDiverged: boolean;
}

const PROMPT_SECTION_REGEX =
  /(?:^|\n)#{1,6}[ \t]*[^\n]*prompt[^\n]*\n([\s\S]*?)(?=\n#{1,6}[ \t]+\S|\s*$)/i;

const extractPromptText = (markdownBody: string): string => {
  if (!markdownBody) return "";
  const sectionMatch = markdownBody.match(PROMPT_SECTION_REGEX);
  const promptContent = sectionMatch?.[1]?.trim();
  return promptContent && promptContent.length > 0 ? promptContent : markdownBody;
};

const compactTranslationPreview = (text: string): string =>
  text
    .replace(/^\s*(翻译内容|Translation|Translated Content)\s*\n+/i, "")
    .replace(/\n{3,}/g, "\n\n");

const isLikelyEnglish = (text: string): boolean => {
  if (!text.trim()) return false;
  const latinCharCount = (text.match(/[A-Za-z]/g) || []).length;
  if (latinCharCount < 8) return false;
  const cjkCharCount = (text.match(/[\u3400-\u9FFF]/g) || []).length;
  return latinCharCount > cjkCharCount * 2;
};

const shortHash = (hash: string) => (hash || "").slice(0, 8);

const markdownComponents: Components = {
  h1: ({ children }) => (
    <h1 className="mt-8 mb-4 border-b border-slate-200 pb-2 text-2xl font-bold text-slate-900 first:mt-0">
      {children}
    </h1>
  ),
  h2: ({ children }) => (
    <h2 className="mt-7 mb-3 text-xl font-semibold text-slate-900 first:mt-0">
      {children}
    </h2>
  ),
  h3: ({ children }) => (
    <h3 className="mt-6 mb-3 text-lg font-semibold text-slate-800 first:mt-0">
      {children}
    </h3>
  ),
  h4: ({ children }) => (
    <h4 className="mt-5 mb-2 text-base font-semibold text-slate-800 first:mt-0">
      {children}
    </h4>
  ),
  p: ({ children }) => <p className="my-3 text-[15px] leading-7 text-slate-700">{children}</p>,
  ul: ({ children }) => (
    <ul className="my-3 list-disc space-y-1 pl-5 text-[15px] leading-7 text-slate-700">
      {children}
    </ul>
  ),
  ol: ({ children }) => (
    <ol className="my-3 list-decimal space-y-1 pl-5 text-[15px] leading-7 text-slate-700">
      {children}
    </ol>
  ),
  li: ({ children }) => <li>{children}</li>,
  blockquote: ({ children }) => (
    <blockquote className="my-4 rounded-r-lg border-l-4 border-indigo-300 bg-indigo-50/70 px-4 py-3 text-slate-700">
      {children}
    </blockquote>
  ),
  hr: () => <hr className="my-6 border-slate-200" />,
  a: ({ href, children }) => (
    <a
      href={href}
      target="_blank"
      rel="noreferrer"
      className="font-medium text-indigo-600 underline decoration-indigo-300 underline-offset-2 transition-colors hover:text-indigo-700"
    >
      {children}
    </a>
  ),
  table: ({ children }) => (
    <div className="my-5 overflow-x-auto rounded-xl border border-slate-200">
      <table className="min-w-full border-collapse text-left text-sm text-slate-700">{children}</table>
    </div>
  ),
  thead: ({ children }) => <thead className="bg-slate-50">{children}</thead>,
  th: ({ children }) => <th className="border-b border-slate-200 px-3 py-2 font-semibold text-slate-700">{children}</th>,
  td: ({ children }) => <td className="border-b border-slate-100 px-3 py-2 align-top">{children}</td>,
  pre: ({ children }) => <>{children}</>,
  code: ({ className, children, ...props }: any) => {
    const language = className?.match(/language-(\w+)/)?.[1];
    const content = String(children).replace(/\n$/, "");
    const isBlockCode = Boolean(language) || content.includes("\n");

    if (!isBlockCode) {
      return (
        <code className="rounded bg-slate-100 px-1.5 py-0.5 font-mono text-[13px] text-indigo-700" {...props}>
          {content}
        </code>
      );
    }
    return (
      <div className="my-4 overflow-hidden rounded-xl border border-slate-200 bg-slate-950">
        {language && (
          <div className="border-b border-slate-800 px-3 py-1.5 text-[11px] font-semibold uppercase tracking-wide text-slate-300">
            {language}
          </div>
        )}
        <pre className="m-0 overflow-x-auto p-4">
          <code
            className={cn(
              className,
              "inline-block min-w-max whitespace-pre font-mono text-[13px] leading-6 text-slate-100"
            )}
            {...props}
          >
            {content}
          </code>
        </pre>
      </div>
    );
  },
};

interface TranslatorStreamEventPayload {
  sessionId: string;
  stage: "start" | "chunk" | "done" | "error";
  chunk?: string;
  result?: string;
  error?: string;
}

interface CoverSkillResponse {
  operationId: string;
  sourceUid: string;
  sourceHash: string;
  sourceVersionLabel: string;
  results: Array<{
    targetAgentType: string;
    targetUid: string;
    targetPath: string;
    action: "updated" | "installed";
    previousHash?: string;
    newHash: string;
    historyEntryId: string;
  }>;
  skipped: Array<{
    targetAgentType: string;
    reason: string;
  }>;
}

interface SkillCoverHistoryEntry {
  entryId: string;
  skillSlug: string;
  targetAgentType: string;
  targetUid: string;
  sourceUid: string;
  sourceNamespace: string;
  sourceVersionLabel: string;
  sourceHash: string;
  previousHash?: string;
  appliedAt: number;
  rolledBackAt?: number | null;
}

export const SkillDetail = ({
  skillGroup,
  agents,
  locale,
  onClose,
  onRefresh,
  onOpenTranslatorSettings,
  onOpenTranslatorGuide,
}: {
  skillGroup: SkillGroup;
  agents: Agent[];
  locale: Locale;
  onClose: () => void;
  onRefresh: () => void;
  onOpenTranslatorSettings: () => void;
  onOpenTranslatorGuide: () => void | Promise<void>;
}) => {
  const [toggling, setToggling] = useState<string | null>(null);
  const [translatedPrompt, setTranslatedPrompt] = useState<string | null>(null);
  const [translatingPrompt, setTranslatingPrompt] = useState(false);
  const [streamModalOpen, setStreamModalOpen] = useState(false);
  const [streamStage, setStreamStage] = useState<"idle" | "running" | "done" | "error">("idle");
  const [streamText, setStreamText] = useState("");
  const [streamError, setStreamError] = useState<string | null>(null);
  const [selectedVariantUid, setSelectedVariantUid] = useState<string>(
    skillGroup.variants[0]?.uid ?? ""
  );
  const [coveringAgent, setCoveringAgent] = useState<string | null>(null);
  const [coveringAll, setCoveringAll] = useState(false);
  const [historyAgentType, setHistoryAgentType] = useState<string | null>(null);
  const [historyEntries, setHistoryEntries] = useState<SkillCoverHistoryEntry[]>([]);
  const [historyLoading, setHistoryLoading] = useState(false);
  const [rollingBackEntryId, setRollingBackEntryId] = useState<string | null>(null);
  const activeSessionRef = useRef<string | null>(null);
  const streamTextRef = useRef<string>("");
  const t = getMessages(locale);

  const localeTag = locale === "zh" ? "zh-CN" : "en-US";

  useEffect(() => {
    setSelectedVariantUid(skillGroup.variants[0]?.uid ?? "");
  }, [skillGroup.id]);

  const selectedVariant = useMemo(() => {
    return (
      skillGroup.variants.find((v) => v.uid === selectedVariantUid) ??
      skillGroup.variants[0]
    );
  }, [selectedVariantUid, skillGroup.variants]);

  const allSameContent = useMemo(() => {
    return new Set(skillGroup.variants.map((v) => v.treeHash)).size <= 1;
  }, [skillGroup.variants]);

  const promptSource = useMemo(
    () => extractPromptText(selectedVariant?.markdownBody ?? ""),
    [selectedVariant?.markdownBody]
  );
  const sourceIsEnglish = useMemo(() => isLikelyEnglish(promptSource), [promptSource]);
  const shouldShowTranslateButton = useMemo(() => promptSource.trim().length > 0, [promptSource]);
  const translateButtonLabel = sourceIsEnglish ? t.translatePromptToChinese : t.translatePromptToEnglish;
  const translatedPromptTitle = sourceIsEnglish ? t.translatedPrompt : t.translatedPromptEnglish;
  const compactStreamText = useMemo(() => compactTranslationPreview(streamText), [streamText]);
  const selectedVariantAgentType = useMemo(() => {
    if (!selectedVariant?.namespace?.startsWith("agent:")) {
      return null;
    }
    return selectedVariant.namespace.replace("agent:", "");
  }, [selectedVariant?.namespace]);

  useEffect(() => {
    setTranslatedPrompt(null);
    setTranslatingPrompt(false);
    setStreamModalOpen(false);
    setStreamStage("idle");
    setStreamText("");
    setStreamError(null);
    setCoveringAgent(null);
    setCoveringAll(false);
    setHistoryAgentType(null);
    setHistoryEntries([]);
    setHistoryLoading(false);
    setRollingBackEntryId(null);
    activeSessionRef.current = null;
    streamTextRef.current = "";
  }, [selectedVariant?.uid]);

  useEffect(() => {
    streamTextRef.current = streamText;
  }, [streamText]);

  useEffect(() => {
    let unlisten: (() => void) | null = null;
    let disposed = false;

    listen<TranslatorStreamEventPayload>("translator-stream", (event) => {
      const payload = event.payload;
      if (payload.sessionId !== activeSessionRef.current) return;

      if (payload.stage === "start") {
        setStreamStage("running");
        setStreamError(null);
        return;
      }
      if (payload.stage === "chunk") {
        if (payload.chunk) {
          setStreamText((prev) => prev + payload.chunk);
        }
        return;
      }
      if (payload.stage === "done") {
        setStreamStage("done");
        if (payload.result && payload.result.trim()) {
          setStreamText(payload.result);
          setTranslatedPrompt(payload.result);
        } else {
          setTranslatedPrompt((prev) => prev ?? streamTextRef.current);
        }
        return;
      }
      if (payload.stage === "error") {
        setStreamStage("error");
        setStreamError(localizeErrorMessage(payload.error ?? "", locale));
      }
    }).then((fn) => {
      if (disposed) {
        fn();
        return;
      }
      unlisten = fn;
    });

    return () => {
      disposed = true;
      if (unlisten) unlisten();
    };
  }, [locale]);

  const toggleSkill = async (agentType: string, isInstalled: boolean) => {
    const installedVariant = skillGroup.variants.find((v) =>
      v.installations.some((i) => i.agentType === agentType)
    );
    const targetVariant = isInstalled ? installedVariant : selectedVariant;
    if (!targetVariant) return;

    setToggling(agentType);
    try {
      await invoke("toggle_skill", {
        skillId: targetVariant.uid,
        agentType,
        install: !isInstalled,
      });
      onRefresh();
    } catch (err) {
      alert(`${t.operationFailedPrefix}: ${localizeErrorMessage(err, locale)}`);
    } finally {
      setToggling(null);
    }
  };

  const showCoverSummary = (response: CoverSkillResponse) => {
    alert(t.coverOperationSummary(response.results.length, response.skipped.length));
  };

  const coverToAgent = async (agentType: string) => {
    setCoveringAgent(agentType);
    try {
      const response = await invoke<CoverSkillResponse>("cover_skill_to_agent", {
        sourceUid: selectedVariant.uid,
        targetAgentType: agentType,
      });
      showCoverSummary(response);
      onRefresh();
      if (historyAgentType === agentType) {
        await loadHistory(agentType);
      }
    } catch (err) {
      alert(`${t.operationFailedPrefix}: ${localizeErrorMessage(err, locale)}`);
    } finally {
      setCoveringAgent(null);
    }
  };

  const coverToAllAvailable = async () => {
    setCoveringAll(true);
    try {
      const response = await invoke<CoverSkillResponse>("cover_skill_to_all_available_agents", {
        sourceUid: selectedVariant.uid,
      });
      showCoverSummary(response);
      onRefresh();
      if (historyAgentType) {
        await loadHistory(historyAgentType);
      }
    } catch (err) {
      alert(`${t.operationFailedPrefix}: ${localizeErrorMessage(err, locale)}`);
    } finally {
      setCoveringAll(false);
    }
  };

  const loadHistory = async (agentType: string) => {
    setHistoryAgentType(agentType);
    setHistoryLoading(true);
    try {
      const entries = await invoke<SkillCoverHistoryEntry[]>("list_skill_cover_history", {
        skillSlug: skillGroup.slug,
        targetAgentType: agentType,
      });
      setHistoryEntries(entries);
    } catch (err) {
      alert(`${t.operationFailedPrefix}: ${localizeErrorMessage(err, locale)}`);
      setHistoryEntries([]);
    } finally {
      setHistoryLoading(false);
    }
  };

  const openHistoryModal = async (agentType: string) => {
    await loadHistory(agentType);
  };

  const closeHistoryModal = () => {
    setHistoryAgentType(null);
    setHistoryEntries([]);
    setHistoryLoading(false);
  };

  const rollbackHistoryEntry = async (entryId: string) => {
    setRollingBackEntryId(entryId);
    try {
      await invoke("rollback_skill_cover_entry", { entryId });
      alert(t.rollbackSuccess);
      onRefresh();
      if (historyAgentType) {
        await loadHistory(historyAgentType);
      }
    } catch (err) {
      alert(`${t.operationFailedPrefix}: ${localizeErrorMessage(err, locale)}`);
    } finally {
      setRollingBackEntryId(null);
    }
  };

  const translatePrompt = async () => {
    if (!promptSource.trim() || !selectedVariant) return;
    setTranslatingPrompt(true);
    setStreamModalOpen(true);
    setStreamStage("running");
    setStreamText("");
    setStreamError(null);
    const sessionId = `${selectedVariant.uid}-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
    activeSessionRef.current = sessionId;

    try {
      await invoke("translate_text_to_zh_stream", {
        sessionId,
        text: promptSource,
      });
    } catch (err) {
      const raw = String(err ?? "");
      if (raw.toLowerCase().includes("translator_config_missing")) {
        setStreamModalOpen(false);
        setStreamStage("idle");
        const goSettings = window.confirm(t.missingTranslatorConfigPrompt);
        if (goSettings) {
          onOpenTranslatorSettings();
          const openGuide = window.confirm(t.openGuidePrompt);
          if (openGuide) {
            await onOpenTranslatorGuide();
          }
        }
        return;
      }
      setStreamStage("error");
      setStreamError(localizeErrorMessage(err, locale));
    } finally {
      setTranslatingPrompt(false);
    }
  };

  if (!selectedVariant) {
    return null;
  }

  return (
    <div className="fixed inset-0 z-50 flex animate-in fade-in duration-200">
      <div className="absolute inset-0 bg-slate-900/40 backdrop-blur-sm" onClick={onClose} />
      <div className="relative ml-auto w-full max-w-6xl bg-white shadow-2xl flex flex-col h-full animate-in slide-in-from-right duration-300">
        <header className="h-16 flex items-center justify-between px-8 border-b border-slate-100 bg-white/80 backdrop-blur-md sticky top-0 z-10">
          <button onClick={onClose} className="flex items-center space-x-2 text-slate-500 hover:text-slate-800 transition-colors">
            <ArrowLeft className="w-4 h-4" />
            <span className="text-sm font-medium">{t.backToLibrary}</span>
          </button>
          <div className="flex items-center space-x-3">
            <button
              onClick={coverToAllAvailable}
              disabled={coveringAll}
              className="px-4 py-1.5 bg-emerald-600 text-white rounded-lg text-sm font-semibold hover:bg-emerald-700 transition-all shadow-lg shadow-emerald-200 disabled:opacity-70 disabled:cursor-wait"
            >
              {coveringAll ? t.coveringToAll : t.coverToAllAvailable}
            </button>
            <button className="px-4 py-1.5 bg-indigo-600 text-white rounded-lg text-sm font-semibold hover:bg-indigo-700 transition-all shadow-lg shadow-indigo-200">
              {t.updateSkill}
            </button>
            <button onClick={onClose} className="p-2 hover:bg-slate-100 rounded-full transition-colors">
              <X className="w-5 h-5 text-slate-400" />
            </button>
          </div>
        </header>

        <div className="flex-1 overflow-y-auto flex">
          <div className="flex-1 min-w-0 p-10 border-r border-slate-50">
            <div className="max-w-3xl mx-auto">
              <div className="mb-8">
                <div className="flex items-center space-x-2 text-[10px] font-bold text-indigo-500 uppercase tracking-widest mb-2">
                  <ShieldCheck className="w-3.5 h-3.5" />
                  <span>{t.verifiedSkill}</span>
                </div>
                <h1 className="text-4xl font-extrabold text-slate-900 mb-2">{skillGroup.displayName || skillGroup.slug}</h1>
                <p className="text-lg text-slate-500 leading-relaxed">
                  {selectedVariant.metadata.description || skillGroup.description}
                </p>
                <div className="mt-3 text-xs font-semibold uppercase tracking-wide">
                  {allSameContent ? (
                    <span className="text-emerald-600">{t.sameVersionAcrossTerminals}</span>
                  ) : (
                    <span className="text-amber-600">{t.multipleTerminalVersionsDetected}</span>
                  )}
                </div>
                {shouldShowTranslateButton && (
                  <button
                    onClick={translatePrompt}
                    disabled={translatingPrompt}
                    className="mt-4 inline-flex items-center gap-1.5 rounded-md bg-blue-600 px-3 py-1.5 text-xs font-semibold text-white shadow-sm transition-colors hover:bg-blue-700 disabled:cursor-wait disabled:opacity-70"
                  >
                    <Languages className="w-3.5 h-3.5" />
                    <span>{translatingPrompt ? t.translating : translateButtonLabel}</span>
                  </button>
                )}
              </div>

              <div className="mb-6 rounded-xl border border-slate-200 bg-slate-50/70 p-4">
                <p className="mb-3 text-xs font-semibold uppercase tracking-wide text-slate-500">
                  {t.terminalVersions}
                </p>
                <div className="flex flex-wrap gap-2">
                  {skillGroup.variants.map((variant) => (
                    <button
                      key={variant.uid}
                      onClick={() => setSelectedVariantUid(variant.uid)}
                      className={cn(
                        "rounded-lg border px-3 py-2 text-left text-xs transition-colors",
                        selectedVariant.uid === variant.uid
                          ? "border-indigo-300 bg-indigo-50 text-indigo-700"
                          : "border-slate-200 bg-white text-slate-600 hover:border-slate-300"
                      )}
                    >
                      <div className="font-semibold">{variant.namespace.replace("agent:", "")}</div>
                      <div className="mt-0.5 text-[11px]">
                        {variant.metadata.version || shortHash(variant.treeHash)}
                      </div>
                    </button>
                  ))}
                </div>
              </div>

              {translatedPrompt && (
                <div className="mb-6 rounded-xl border border-blue-100 bg-blue-50/60 p-4">
                  <div className="mb-2 flex items-center justify-between">
                    <h3 className="text-sm font-semibold text-blue-800">{translatedPromptTitle}</h3>
                    <span className="text-[11px] text-blue-600">{t.machineTranslationDisclaimer}</span>
                  </div>
                  <div className="break-words">
                    <ReactMarkdown remarkPlugins={[remarkGfm]} components={markdownComponents}>
                      {translatedPrompt}
                    </ReactMarkdown>
                  </div>
                </div>
              )}

              <div className="break-words">
                <ReactMarkdown remarkPlugins={[remarkGfm]} components={markdownComponents}>
                  {selectedVariant.markdownBody}
                </ReactMarkdown>
              </div>
            </div>
          </div>

          <div className="w-96 min-w-96 shrink-0 bg-slate-50/50 p-6 flex flex-col">
            <h3 className="text-sm font-bold text-slate-400 uppercase tracking-widest mb-6 flex items-center">
              <Download className="w-4 h-4 mr-2" />
              {t.installation}
            </h3>

            <div className="space-y-3">
              {agents.map((agent) => {
                const directInstalledVariant = skillGroup.variants.find((v) =>
                  v.installations.some((i) => i.agentType === agent.agentType && !i.isInherited)
                );
                const inheritedVariant = skillGroup.variants.find((v) =>
                  v.installations.some((i) => i.agentType === agent.agentType && i.isInherited)
                );
                const effectiveVariant = directInstalledVariant ?? inheritedVariant;
                const isInstalled = !!directInstalledVariant;
                const isInheritedOnly = !isInstalled && !!inheritedVariant;
                const isUpdating = toggling === agent.agentType;
                const isCovering = coveringAgent === agent.agentType;
                const isTargetAvailable =
                  agent.isInstalled || agent.skillsDirectoryExists || agent.skillCount > 0 || !!effectiveVariant;
                const canCover =
                  isTargetAvailable && selectedVariantAgentType !== agent.agentType;
                const status = getAgentStatusMeta(agent, locale);

                return (
                  <div key={agent.agentType} className="bg-white p-4 rounded-2xl border border-slate-200/60 shadow-sm">
                    <div className="min-w-0 flex items-start gap-3">
                      <div className={cn("mt-1.5 h-2.5 w-2.5 shrink-0 rounded-full", status.dotClassName)} />
                      <div className="min-w-0 flex-1">
                        <div className="flex flex-wrap items-center gap-2">
                          <span className="text-sm font-semibold capitalize text-slate-700">{formatAgentName(agent.agentType)}</span>
                          <span className={cn("shrink-0 whitespace-nowrap rounded-full px-2 py-0.5 text-[10px] font-semibold", status.badgeClassName)}>
                            {status.label}
                          </span>
                        </div>
                        <p className="mt-1 break-all text-[11px] text-slate-400">
                          {effectiveVariant
                            ? `${effectiveVariant.namespace} · ${effectiveVariant.metadata.version || shortHash(effectiveVariant.treeHash)}${isInheritedOnly ? ` · ${t.inheritedVersion}` : ""}`
                            : status.description}
                        </p>
                      </div>
                    </div>
                    <div className="mt-3 flex flex-wrap items-center justify-end gap-2">
                      {canCover && (
                        <button
                          disabled={isCovering}
                          onClick={() => coverToAgent(agent.agentType)}
                          className={cn(
                            "whitespace-nowrap rounded-full bg-emerald-50 px-3 py-1 text-[11px] font-semibold text-emerald-700 transition-all hover:bg-emerald-100",
                            isCovering && "opacity-50 cursor-wait"
                          )}
                        >
                          {isCovering ? t.coveringToAgent : t.coverToAgent}
                        </button>
                      )}
                      <button
                        disabled={isUpdating}
                        onClick={() => toggleSkill(agent.agentType, isInstalled)}
                        className={cn(
                          "whitespace-nowrap rounded-full px-3 py-1 text-[11px] font-semibold transition-all",
                          isInstalled
                            ? "bg-red-50 text-red-600 hover:bg-red-100"
                            : "bg-indigo-50 text-indigo-600 hover:bg-indigo-100",
                          isUpdating && "opacity-50 cursor-wait"
                        )}
                      >
                        {isUpdating ? "..." : isInstalled ? t.remove : t.install}
                      </button>
                    </div>
                    <div className="mt-3 flex flex-wrap items-center justify-between gap-2">
                      {isInstalled ? (
                        <div className="flex items-center text-[10px] font-medium text-green-600">
                          <Check className="mr-1 h-3 w-3" />
                          <span>{allSameContent ? t.sameVersion : t.specificVersion}</span>
                        </div>
                      ) : isInheritedOnly ? (
                        <div className="text-[10px] font-medium text-amber-600">{t.inheritedVersion}</div>
                      ) : (
                        <div className="text-[10px] text-slate-400">{status.label}</div>
                      )}
                      <div className="flex items-center gap-2">
                        <button
                          onClick={() => openHistoryModal(agent.agentType)}
                          className={cn(
                            "inline-flex items-center gap-1 whitespace-nowrap rounded-full px-2 py-1 text-[10px] font-semibold transition-colors",
                            historyAgentType === agent.agentType
                              ? "bg-slate-200 text-slate-700"
                              : "bg-slate-100 text-slate-600 hover:bg-slate-200"
                          )}
                        >
                          <History className="h-3 w-3" />
                          <span>{t.coverHistory}</span>
                        </button>
                        {agent.skillCount > 0 && (
                          <div className="text-[10px] text-slate-400">{t.statusSkillCount(agent.skillCount)}</div>
                        )}
                      </div>
                    </div>
                  </div>
                );
              })}
            </div>

            <div className="mt-auto pt-8 border-t border-slate-200 text-center">
              <div className="flex items-center justify-center space-x-1 text-slate-400 mb-2">
                <Info className="w-3.5 h-3.5" />
                <span className="text-[11px]">{t.skillId}: {skillGroup.slug}</span>
              </div>
              <p className="text-[10px] text-slate-300">
                {t.activeVersionPrefix}: {selectedVariant.namespace} · {selectedVariant.metadata.version || shortHash(selectedVariant.treeHash)}
              </p>
            </div>
          </div>
        </div>
      </div>
      {streamModalOpen && (
        <div className="absolute inset-0 z-[70] flex items-center justify-center bg-slate-900/45 backdrop-blur-sm px-4">
          <div className="w-full max-w-2xl overflow-hidden rounded-2xl border border-slate-200 bg-white shadow-2xl">
            <div className="flex items-center justify-between border-b border-slate-100 px-5 py-4">
              <div>
                <h3 className="text-sm font-semibold text-slate-800">{t.liveTranslationTitle}</h3>
                <p className="mt-1 text-xs text-slate-500">{t.liveTranslationHint}</p>
              </div>
              <span className={cn(
                "rounded-full px-2.5 py-1 text-[11px] font-semibold",
                streamStage === "running" && "bg-blue-100 text-blue-700",
                streamStage === "done" && "bg-emerald-100 text-emerald-700",
                streamStage === "error" && "bg-red-100 text-red-700",
                streamStage === "idle" && "bg-slate-100 text-slate-600"
              )}>
                {streamStage === "running" && t.liveTranslationRunning}
                {streamStage === "done" && t.liveTranslationDone}
                {streamStage === "error" && t.liveTranslationError}
                {streamStage === "idle" && "..."}
              </span>
            </div>
            <div className="px-5 py-4">
              <div className="max-h-[48vh] min-h-[220px] overflow-y-auto rounded-xl bg-slate-900 p-4 font-mono text-sm leading-6 text-slate-100 whitespace-pre-wrap">
                {compactStreamText || ""}
                {streamStage === "running" && (
                  <span className="ml-1 inline-block h-4 w-2 animate-pulse bg-cyan-300 align-middle" />
                )}
              </div>
              {streamError && (
                <p className="mt-3 text-xs text-red-600">{streamError}</p>
              )}
            </div>
            <div className="flex justify-end border-t border-slate-100 px-5 py-3">
              <button
                onClick={() => setStreamModalOpen(false)}
                className="rounded-lg bg-slate-100 px-3 py-1.5 text-xs font-medium text-slate-700 transition-colors hover:bg-slate-200"
              >
                {t.closeModal}
              </button>
            </div>
          </div>
        </div>
      )}
      {historyAgentType && (
        <div className="absolute inset-0 z-[75] flex items-center justify-center bg-slate-900/45 backdrop-blur-sm px-4">
          <div className="w-full max-w-2xl overflow-hidden rounded-2xl border border-slate-200 bg-white shadow-2xl">
            <div className="flex items-center justify-between border-b border-slate-100 px-5 py-4">
              <div>
                <h3 className="text-sm font-semibold text-slate-800">{t.coverHistory}</h3>
                <p className="mt-1 text-xs text-slate-500">
                  {formatAgentName(historyAgentType)}
                </p>
              </div>
              <button
                onClick={closeHistoryModal}
                className="rounded-full p-1.5 text-slate-500 hover:bg-slate-100 hover:text-slate-700"
              >
                <X className="h-4 w-4" />
              </button>
            </div>
            <div className="max-h-[56vh] overflow-y-auto px-5 py-4">
              {historyLoading ? (
                <div className="text-sm text-slate-500">{t.loadingHistory}</div>
              ) : historyEntries.length === 0 ? (
                <div className="text-sm text-slate-500">{t.noCoverHistory}</div>
              ) : (
                <div className="space-y-2">
                  {historyEntries.map((entry) => (
                    <div key={entry.entryId} className="rounded-lg border border-slate-200 bg-slate-50 p-3">
                      <div className="flex items-center justify-between gap-2">
                        <span className="text-sm font-semibold text-slate-800">{entry.sourceVersionLabel}</span>
                        <span className="text-xs text-slate-500">
                          {new Date(entry.appliedAt * 1000).toLocaleString(localeTag)}
                        </span>
                      </div>
                      <div className="mt-1 text-xs text-slate-500">{entry.sourceNamespace}</div>
                      <div className="mt-2 flex justify-end">
                        <button
                          onClick={() => rollbackHistoryEntry(entry.entryId)}
                          disabled={rollingBackEntryId === entry.entryId}
                          className={cn(
                            "rounded-full px-3 py-1.5 text-[11px] font-semibold transition-colors",
                            entry.rolledBackAt
                              ? "bg-slate-100 text-slate-500"
                              : "bg-amber-100 text-amber-700 hover:bg-amber-200",
                            rollingBackEntryId === entry.entryId && "opacity-50 cursor-wait"
                          )}
                        >
                          {rollingBackEntryId === entry.entryId
                            ? "..."
                            : entry.rolledBackAt
                            ? `${t.rollbackToThisVersion} ✓`
                            : t.rollbackToThisVersion}
                        </button>
                      </div>
                    </div>
                  ))}
                </div>
              )}
            </div>
          </div>
        </div>
      )}
    </div>
  );
};
