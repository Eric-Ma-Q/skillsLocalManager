import React, { useEffect, useMemo, useRef, useState } from "react";
import ReactMarkdown from "react-markdown";
import type { Components } from "react-markdown";
import remarkGfm from "remark-gfm";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { ArrowLeft, Clock3, Download, Languages, ShieldCheck, Star, UserRound, X } from "lucide-react";
import { clsx, type ClassValue } from "clsx";
import { twMerge } from "tailwind-merge";
import { formatAgentName, getAgentStatusMeta, type Agent } from "../agentStatus";
import { getMessages, localizeErrorMessage, type Locale } from "../i18n";
import type { RegistrySkillDetail as RegistrySkillDetailData, SkillGroup } from "../types";

function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

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

interface RegistrySkillDetailProps {
  agents: Agent[];
  detail: RegistrySkillDetailData;
  installing: boolean;
  isTransitioningToLocal?: boolean;
  localSkill?: SkillGroup | null;
  locale: Locale;
  onClose: () => void;
  onInstall: () => void;
  onOpenLocalSkill?: () => void;
  onOpenTranslatorGuide: () => void | Promise<void>;
  onOpenTranslatorSettings: () => void;
}

const formatTimestamp = (value: number | null | undefined, locale: Locale) => {
  if (!value) return "-";
  return new Date(value).toLocaleString(locale === "zh" ? "zh-CN" : "en-US");
};

const formatNumber = (value: number, locale: Locale) =>
  new Intl.NumberFormat(locale === "zh" ? "zh-CN" : "en-US", {
    notation: value >= 10000 ? "compact" : "standard",
    maximumFractionDigits: 1,
  }).format(value);

export const RegistrySkillDetailPanel = ({
  agents,
  detail,
  installing,
  isTransitioningToLocal = false,
  localSkill,
  locale,
  onClose,
  onInstall,
  onOpenLocalSkill,
  onOpenTranslatorGuide,
  onOpenTranslatorSettings,
}: RegistrySkillDetailProps) => {
  const [translatedPrompt, setTranslatedPrompt] = useState<string | null>(null);
  const [translationView, setTranslationView] = useState<"translated" | "source">("translated");
  const [translatingPrompt, setTranslatingPrompt] = useState(false);
  const [streamModalOpen, setStreamModalOpen] = useState(false);
  const [streamStage, setStreamStage] = useState<"idle" | "running" | "done" | "error">("idle");
  const [streamText, setStreamText] = useState("");
  const [streamError, setStreamError] = useState<string | null>(null);
  const activeSessionRef = useRef<string | null>(null);
  const streamTextRef = useRef<string>("");
  const t = getMessages(locale);

  const translationSource = useMemo(() => (detail.markdownBody || "").trim(), [detail.markdownBody]);
  const sourceIsEnglish = useMemo(() => isLikelyEnglish(translationSource), [translationSource]);
  const shouldShowTranslateButton = useMemo(
    () => translationSource.trim().length > 0,
    [translationSource]
  );
  const translateButtonLabel = sourceIsEnglish ? t.translatePromptToChinese : t.translatePromptToEnglish;
  const translatedPromptTitle = sourceIsEnglish ? t.translatedPrompt : t.translatedPromptEnglish;
  const compactStreamText = useMemo(() => compactTranslationPreview(streamText), [streamText]);
  const installedAgents = useMemo(() => {
    if (!localSkill) return [];
    return Array.from(new Set(localSkill.installations.map((installation: any) => installation.agentType))).sort();
  }, [localSkill]);
  const localVersionLabels = useMemo(() => {
    if (!localSkill) return [];
    return Array.from(
      new Set(
        localSkill.variants.map((variant) => variant.metadata.version || variant.remoteVersionLabel || variant.treeHash.slice(0, 8))
      )
    );
  }, [localSkill]);
  const hasLocalUpdate = Boolean(localSkill?.variants.some((variant) => variant.hasUpdate));
  const agentInstallStates = useMemo(() => {
    return agents.map((agent) => {
      const directInstalledVariant = localSkill?.variants.find((variant) =>
        variant.installations.some((installation: any) => installation.agentType === agent.agentType && !installation.isInherited)
      );
      const inheritedVariant = localSkill?.variants.find((variant) =>
        variant.installations.some((installation: any) => installation.agentType === agent.agentType && installation.isInherited)
      );
      const effectiveVariant = directInstalledVariant ?? inheritedVariant ?? null;
      const isInstalled = Boolean(directInstalledVariant);
      const isInheritedOnly = !isInstalled && Boolean(inheritedVariant);
      const status = getAgentStatusMeta(agent, locale);
      return {
        agent,
        status,
        effectiveVariant,
        isInstalled,
        isInheritedOnly,
      };
    });
  }, [agents, localSkill, locale]);

  useEffect(() => {
    setTranslatedPrompt(null);
    setTranslationView("translated");
    setTranslatingPrompt(false);
    setStreamModalOpen(false);
    setStreamStage("idle");
    setStreamText("");
    setStreamError(null);
    activeSessionRef.current = null;
    streamTextRef.current = "";
  }, [detail.slug]);

  useEffect(() => {
    if (translatedPrompt) {
      setTranslationView("translated");
    }
  }, [translatedPrompt]);

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
          setTranslationView("translated");
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

  const translatePrompt = async () => {
    if (!translationSource.trim()) return;
    setTranslatingPrompt(true);
    setStreamModalOpen(true);
    setStreamStage("running");
    setStreamText("");
    setStreamError(null);
    const sessionId = `${detail.slug}-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
    activeSessionRef.current = sessionId;

    try {
      await invoke("translate_text_to_zh_stream", {
        sessionId,
        text: translationSource,
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

  return (
    <div
      className={cn(
        "fixed inset-0 z-[60] flex duration-200",
        isTransitioningToLocal ? "animate-out fade-out" : "animate-in fade-in"
      )}
    >
      <div className="absolute inset-0 bg-slate-900/40 backdrop-blur-sm" onClick={onClose} />
      <div
        className={cn(
          "relative ml-auto flex h-full w-full max-w-6xl flex-col bg-white shadow-2xl duration-300",
          isTransitioningToLocal ? "animate-out slide-out-to-right" : "animate-in slide-in-from-right"
        )}
      >
        <header className="sticky top-0 z-10 flex h-16 items-center justify-between border-b border-slate-100 bg-white/80 px-8 backdrop-blur-md">
          <button onClick={onClose} className="flex items-center space-x-2 text-slate-500 transition-colors hover:text-slate-800">
            <ArrowLeft className="h-4 w-4" />
            <span className="text-sm font-medium">{t.backToLibrary}</span>
          </button>
          <div className="flex items-center gap-3">
            <button
              onClick={onInstall}
              disabled={installing}
              className="rounded-lg bg-indigo-600 px-4 py-1.5 text-sm font-semibold text-white shadow-lg shadow-indigo-200 transition-all hover:bg-indigo-700 disabled:cursor-wait disabled:opacity-70"
            >
              {installing ? t.working : t.install}
            </button>
            <button onClick={onClose} className="rounded-full p-2 transition-colors hover:bg-slate-100">
              <X className="h-5 w-5 text-slate-400" />
            </button>
          </div>
        </header>

        <div className="flex flex-1 overflow-y-auto">
          <div className="min-w-0 flex-1 border-r border-slate-50 p-10">
            <div className="mx-auto max-w-3xl">
              <div className="mb-8">
                <div className="mb-2 flex items-center space-x-2 text-[10px] font-bold uppercase tracking-widest text-indigo-500">
                  <ShieldCheck className="h-3.5 w-3.5" />
                  <span>{t.registrySourceLabel}</span>
                </div>
                <h1 className="mb-2 text-4xl font-extrabold text-slate-900">{detail.displayName}</h1>
                <p className="text-lg leading-relaxed text-slate-500">{detail.summary}</p>
                <div className="mt-3 flex flex-wrap items-center gap-2 text-[11px] font-semibold">
                  <span className="rounded-full bg-slate-100 px-2.5 py-1 text-slate-600">/{detail.slug}</span>
                  <span className="rounded-full bg-indigo-50 px-2.5 py-1 text-indigo-700">
                    {detail.latestVersion ? `v${detail.latestVersion}` : t.latestVersionUnknown}
                  </span>
                </div>
                {shouldShowTranslateButton && (
                  <button
                    onClick={translatePrompt}
                    disabled={translatingPrompt}
                    className="mt-4 inline-flex items-center gap-1.5 rounded-md bg-blue-600 px-3 py-1.5 text-xs font-semibold text-white shadow-sm transition-colors hover:bg-blue-700 disabled:cursor-wait disabled:opacity-70"
                  >
                    <Languages className="h-3.5 w-3.5" />
                    <span>{translatingPrompt ? t.translating : translateButtonLabel}</span>
                  </button>
                )}
              </div>

              {translatedPrompt && (
                <div className="mb-6 rounded-xl border border-blue-100 bg-blue-50/60 p-4">
                  <div className="mb-3 flex items-center justify-between gap-3">
                    <div className="inline-flex rounded-full border border-blue-200 bg-white p-1">
                      <button
                        onClick={() => setTranslationView("translated")}
                        className={cn(
                          "rounded-full px-3 py-1 text-[11px] font-semibold transition-colors",
                          translationView === "translated"
                            ? "bg-blue-600 text-white"
                            : "text-blue-700 hover:bg-blue-50"
                        )}
                      >
                        {t.translatedPromptTab}
                      </button>
                      <button
                        onClick={() => setTranslationView("source")}
                        className={cn(
                          "rounded-full px-3 py-1 text-[11px] font-semibold transition-colors",
                          translationView === "source"
                            ? "bg-blue-600 text-white"
                            : "text-blue-700 hover:bg-blue-50"
                        )}
                      >
                        {t.originalPrompt}
                      </button>
                    </div>
                    <span className="text-[11px] text-blue-600">{t.machineTranslationDisclaimer}</span>
                  </div>
                  <h3 className="mb-2 text-sm font-semibold text-blue-800">
                    {translationView === "translated" ? translatedPromptTitle : t.originalPrompt}
                  </h3>
                  <div className="break-words">
                    <ReactMarkdown remarkPlugins={[remarkGfm]} components={markdownComponents}>
                      {translationView === "translated" ? translatedPrompt : translationSource}
                    </ReactMarkdown>
                  </div>
                </div>
              )}

              <div className="break-words">
                <ReactMarkdown remarkPlugins={[remarkGfm]} components={markdownComponents}>
                  {detail.markdownBody || ""}
                </ReactMarkdown>
              </div>
            </div>
          </div>

          <aside className="flex w-96 min-w-96 shrink-0 flex-col bg-slate-50/50 p-6">
            <h3 className="mb-6 flex items-center text-sm font-bold uppercase tracking-widest text-slate-400">
              <Download className="mr-2 h-4 w-4" />
              {t.installation}
            </h3>

            <div className="space-y-4">
              <div className="rounded-2xl border border-slate-200/60 bg-white p-5 shadow-sm">
                <h4 className="text-sm font-semibold text-slate-800">{t.install}</h4>
                <p className="mt-2 text-xs leading-6 text-slate-500">
                  {t.registryInstallHint}
                </p>
                <button
                  onClick={onInstall}
                  disabled={installing}
                  className="mt-4 w-full rounded-xl bg-indigo-600 px-4 py-2.5 text-sm font-semibold text-white transition-colors hover:bg-indigo-700 disabled:cursor-wait disabled:opacity-70"
                >
                  {installing ? t.working : t.install}
                </button>
              </div>

              <div className="rounded-2xl border border-slate-200/60 bg-white p-5 shadow-sm">
                <h4 className="text-sm font-semibold text-slate-800">{t.localStateTitle}</h4>
                {localSkill ? (
                  <>
                    <div className="mt-3 flex flex-wrap items-center gap-2">
                      <span className="rounded-full bg-emerald-50 px-2.5 py-1 text-[11px] font-semibold text-emerald-700">
                        {t.installedLocally}
                      </span>
                      {hasLocalUpdate && (
                        <span className="rounded-full bg-amber-50 px-2.5 py-1 text-[11px] font-semibold text-amber-700">
                          {t.updateAvailableBadge}
                        </span>
                      )}
                      {localSkill.hasDiverged && (
                        <span className="rounded-full bg-rose-50 px-2.5 py-1 text-[11px] font-semibold text-rose-700">
                          {t.versionsDivergedBadge}
                        </span>
                      )}
                    </div>
                    <p className="mt-3 text-xs leading-6 text-slate-500">
                      {t.installedOnAgents(installedAgents.length)}
                    </p>
                    {installedAgents.length > 0 && (
                      <div className="mt-3 flex flex-wrap gap-2">
                        {installedAgents.map((agentType) => (
                          <span
                            key={agentType}
                            className="rounded-full bg-slate-100 px-2.5 py-1 text-[11px] font-semibold capitalize text-slate-600"
                          >
                            {agentType}
                          </span>
                        ))}
                      </div>
                    )}
                    <div className="mt-4">
                      <div className="mb-2 text-[11px] font-semibold uppercase tracking-wide text-slate-400">
                        {t.localVersionsLabel}
                      </div>
                      <div className="flex flex-wrap gap-2">
                        {localVersionLabels.map((versionLabel) => (
                          <span
                            key={versionLabel}
                            className="rounded-full bg-indigo-50 px-2.5 py-1 text-[11px] font-semibold text-indigo-700"
                          >
                            {versionLabel}
                          </span>
                        ))}
                      </div>
                    </div>
                    {onOpenLocalSkill && (
                      <button
                        onClick={onOpenLocalSkill}
                        className="mt-4 w-full rounded-xl border border-slate-200 px-4 py-2.5 text-sm font-semibold text-slate-700 transition-colors hover:bg-slate-50"
                      >
                        {t.openLocalDetail}
                      </button>
                    )}
                  </>
                ) : (
                  <div className="mt-3">
                    <span className="rounded-full bg-slate-100 px-2.5 py-1 text-[11px] font-semibold text-slate-600">
                      {t.notInstalledLocally}
                    </span>
                  </div>
                )}
              </div>

              <div className="rounded-2xl border border-slate-200/60 bg-white p-5 shadow-sm">
                <h4 className="text-sm font-semibold text-slate-800">{t.terminalStatusTitle}</h4>
                <div className="mt-4 space-y-3">
                  {agentInstallStates.map(({ agent, status, effectiveVariant, isInstalled, isInheritedOnly }) => (
                    <div key={agent.agentType} className="rounded-2xl border border-slate-200/70 bg-slate-50/70 p-3">
                      <div className="flex items-start justify-between gap-3">
                        <div className="min-w-0">
                          <div className="flex flex-wrap items-center gap-2">
                            <span className="text-sm font-semibold capitalize text-slate-800">
                              {formatAgentName(agent.agentType)}
                            </span>
                            <span
                              className={cn(
                                "rounded-full px-2 py-0.5 text-[10px] font-semibold",
                                status.badgeClassName
                              )}
                            >
                              {status.label}
                            </span>
                          </div>
                          <p className="mt-1 break-all text-[11px] text-slate-500">
                            {effectiveVariant
                              ? `${effectiveVariant.namespace} · ${effectiveVariant.metadata.version || effectiveVariant.treeHash.slice(0, 8)}`
                              : status.description}
                          </p>
                        </div>
                        <span
                          className={cn(
                            "shrink-0 rounded-full px-2.5 py-1 text-[10px] font-semibold",
                            isInstalled && "bg-emerald-50 text-emerald-700",
                            isInheritedOnly && "bg-amber-50 text-amber-700",
                            !isInstalled && !isInheritedOnly && "bg-slate-100 text-slate-600"
                          )}
                        >
                          {isInstalled
                            ? t.installedDirectly
                            : isInheritedOnly
                              ? t.installedInherited
                              : t.notInstalledOnTerminal}
                        </span>
                      </div>
                    </div>
                  ))}
                </div>
              </div>

              <div className="rounded-2xl border border-slate-200/60 bg-white p-5 shadow-sm">
                <h4 className="text-sm font-semibold text-slate-800">{t.about}</h4>
                <div className="mt-4 space-y-3">
                  <div className="flex items-start justify-between gap-4">
                    <span className="text-xs text-slate-400">{t.skillId}</span>
                    <span className="text-right text-xs font-medium text-slate-700">/{detail.slug}</span>
                  </div>
                  <div className="flex items-start justify-between gap-4">
                    <span className="text-xs text-slate-400">{t.remoteVersionLabel}</span>
                    <span className="text-right text-xs font-medium text-slate-700">
                      {detail.latestVersion ? `v${detail.latestVersion}` : t.latestVersionUnknown}
                    </span>
                  </div>
                  <div className="flex items-start justify-between gap-4">
                    <span className="inline-flex items-center gap-1 text-xs text-slate-400">
                      <Download className="h-3.5 w-3.5" />
                      {t.downloadsLabel}
                    </span>
                    <span className="text-right text-xs font-medium text-slate-700">
                      {formatNumber(detail.downloads, locale)}
                    </span>
                  </div>
                  <div className="flex items-start justify-between gap-4">
                    <span className="inline-flex items-center gap-1 text-xs text-slate-400">
                      <Star className="h-3.5 w-3.5" />
                      {t.starsLabel}
                    </span>
                    <span className="text-right text-xs font-medium text-slate-700">
                      {formatNumber(detail.stars, locale)}
                    </span>
                  </div>
                  <div className="flex items-start justify-between gap-4">
                    <span className="inline-flex items-center gap-1 text-xs text-slate-400">
                      <Clock3 className="h-3.5 w-3.5" />
                      {t.sortUpdated}
                    </span>
                    <span className="text-right text-xs font-medium text-slate-700">
                      {formatTimestamp(detail.updatedAt, locale)}
                    </span>
                  </div>
                  <div className="flex items-start justify-between gap-4">
                    <span className="text-xs text-slate-400">{t.createdLabel}</span>
                    <span className="text-right text-xs font-medium text-slate-700">
                      {formatTimestamp(detail.createdAt, locale)}
                    </span>
                  </div>
                  <div className="flex items-start justify-between gap-4">
                    <span className="inline-flex items-center gap-1 text-xs text-slate-400">
                      <UserRound className="h-3.5 w-3.5" />
                      {t.authorLabel}
                    </span>
                    <span className="text-right text-xs font-medium text-slate-700">
                      {detail.owner?.displayName || detail.owner?.handle || "-"}
                    </span>
                  </div>
                </div>
              </div>
            </div>
          </aside>
        </div>
      </div>

      {streamModalOpen && (
        <div className="absolute inset-0 z-[70] flex items-center justify-center bg-slate-900/45 px-4 backdrop-blur-sm">
          <div className="w-full max-w-2xl overflow-hidden rounded-2xl border border-slate-200 bg-white shadow-2xl">
            <div className="flex items-center justify-between border-b border-slate-100 px-5 py-4">
              <div>
                <h3 className="text-sm font-semibold text-slate-800">{t.liveTranslationTitle}</h3>
                <p className="mt-1 text-xs text-slate-500">{t.liveTranslationHint}</p>
              </div>
              <span
                className={cn(
                  "rounded-full px-2.5 py-1 text-[11px] font-semibold",
                  streamStage === "running" && "bg-blue-100 text-blue-700",
                  streamStage === "done" && "bg-emerald-100 text-emerald-700",
                  streamStage === "error" && "bg-red-100 text-red-700",
                  streamStage === "idle" && "bg-slate-100 text-slate-600"
                )}
              >
                {streamStage === "running" && t.liveTranslationRunning}
                {streamStage === "done" && t.liveTranslationDone}
                {streamStage === "error" && t.liveTranslationError}
                {streamStage === "idle" && "..."}
              </span>
            </div>
            <div className="px-5 py-4">
              <div className="max-h-[48vh] min-h-[220px] whitespace-pre-wrap rounded-xl bg-slate-900 p-4 font-mono text-sm leading-6 text-slate-100">
                {compactStreamText || ""}
                {streamStage === "running" && (
                  <span className="ml-1 inline-block h-4 w-2 animate-pulse bg-cyan-300 align-middle" />
                )}
              </div>
              {streamError && <p className="mt-3 text-xs text-red-600">{streamError}</p>}
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
    </div>
  );
};
