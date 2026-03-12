import React, { useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-shell";
import { CheckCircle2, ExternalLink, FolderPlus, Sparkles, X } from "lucide-react";
import { clsx, type ClassValue } from "clsx";
import { twMerge } from "tailwind-merge";
import { getMessages, localizeErrorMessage, type Locale } from "../i18n";

function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

const ONBOARDING_KEY = "skillsLocalManager.onboarding.claudeInit.v1";
const DOCS_URL = "https://support.claude.com/en/articles/12512180-using-skills-in-claude";

interface ClaudeBootstrapSkill {
  slug: string;
  name: string;
  description: string;
  recommended: boolean;
}

interface ClaudeBootstrapCatalog {
  targetDir: string;
  targetDirExists: boolean;
  canCreateTargetDir: boolean;
  claudeCliInstalled: boolean;
  recommendedSkills: ClaudeBootstrapSkill[];
  optionalSkills: ClaudeBootstrapSkill[];
  existingSkillSlugs: string[];
}

interface ClaudeBootstrapResult {
  targetDir: string;
  createdTargetDir: boolean;
  installed: string[];
  skipped: Array<{
    slug: string;
    reason: string;
  }>;
  sourceRepo: string;
  sourceRef: string;
}

interface ClaudeInitBannerProps {
  enabled: boolean;
  locale: Locale;
  onInstalled: () => void;
}

export const ClaudeInitBanner: React.FC<ClaudeInitBannerProps> = ({
  enabled,
  locale,
  onInstalled,
}) => {
  const t = getMessages(locale);
  const [state, setState] = useState<"pending" | "dismissed" | "completed">("pending");
  const [modalOpen, setModalOpen] = useState(false);
  const [catalog, setCatalog] = useState<ClaudeBootstrapCatalog | null>(null);
  const [selectedSlugs, setSelectedSlugs] = useState<string[]>([]);
  const [loadingCatalog, setLoadingCatalog] = useState(false);
  const [installing, setInstalling] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [result, setResult] = useState<ClaudeBootstrapResult | null>(null);

  useEffect(() => {
    const saved = window.localStorage.getItem(ONBOARDING_KEY);
    if (saved === "dismissed" || saved === "completed") {
      setState(saved);
    } else {
      setState("pending");
    }
  }, []);

  useEffect(() => {
    if (!enabled || state !== "pending") {
      setModalOpen(false);
    }
  }, [enabled, state]);

  useEffect(() => {
    if (!enabled || state !== "pending" || catalog || loadingCatalog) {
      return;
    }

    setLoadingCatalog(true);
    setError(null);
    invoke<ClaudeBootstrapCatalog>("get_claude_bootstrap_catalog")
      .then((res) => {
        setCatalog(res);
        setSelectedSlugs(res.recommendedSkills.map((skill) => skill.slug));
      })
      .catch((err) => {
        setError(localizeErrorMessage(err, locale));
      })
      .finally(() => {
        setLoadingCatalog(false);
      });
  }, [catalog, enabled, loadingCatalog, locale, state]);

  const visible = enabled && state === "pending";
  const catalogSkills = useMemo(() => {
    if (!catalog) return [];
    return [...catalog.recommendedSkills, ...catalog.optionalSkills];
  }, [catalog]);

  const toggleSkill = (slug: string) => {
    setSelectedSlugs((prev) =>
      prev.includes(slug) ? prev.filter((item) => item !== slug) : [...prev, slug]
    );
  };

  const handleDismiss = () => {
    window.localStorage.setItem(ONBOARDING_KEY, "dismissed");
    setState("dismissed");
    setModalOpen(false);
  };

  const openDocs = async () => {
    try {
      await open(DOCS_URL);
    } catch {
      window.open(DOCS_URL, "_blank");
    }
  };

  const openModal = () => {
    setModalOpen(true);
    setError(null);
    setResult(null);
  };

  const installButtonLabel = catalog?.targetDirExists
    ? t.claudeInitInstall
    : t.claudeInitCreateAndInstall;

  const handleInstall = async () => {
    if (selectedSlugs.length === 0) {
      setError(t.claudeInitNoSkillsSelected);
      return;
    }

    setInstalling(true);
    setError(null);
    try {
      const installResult = await invoke<ClaudeBootstrapResult>(
        "install_claude_bootstrap_skills",
        {
          request: {
            skillSlugs: selectedSlugs,
            createTargetDirIfMissing: true,
          },
        }
      );
      setResult(installResult);
      window.localStorage.setItem(ONBOARDING_KEY, "completed");
      setState("completed");
      await onInstalled();
    } catch (err) {
      setError(localizeErrorMessage(err, locale));
    } finally {
      setInstalling(false);
    }
  };

  if (!visible) {
    return (
      <>
        {modalOpen && (
          <div className="fixed inset-0 z-50 flex items-center justify-center bg-slate-950/45 p-6">
            <div className="w-full max-w-3xl rounded-3xl border border-slate-200 bg-white shadow-2xl">
              <div className="flex items-center justify-between border-b border-slate-200 px-6 py-5">
                <div>
                  <h3 className="text-lg font-semibold text-slate-900">{t.claudeInitModalTitle}</h3>
                  <p className="mt-1 text-sm text-slate-500">{t.claudeInitModalSubtitle}</p>
                </div>
                <button
                  onClick={() => setModalOpen(false)}
                  className="rounded-full p-2 text-slate-400 transition-colors hover:bg-slate-100 hover:text-slate-600"
                >
                  <X className="h-4 w-4" />
                </button>
              </div>
              <div className="px-6 py-5">
                {result && (
                  <div className="rounded-2xl border border-emerald-200 bg-emerald-50 p-4 text-sm text-emerald-900">
                    <div className="flex items-center gap-2 font-semibold">
                      <CheckCircle2 className="h-4 w-4" />
                      <span>{t.claudeInitInstalledSummary(result.installed.length, result.skipped.length)}</span>
                    </div>
                    <p className="mt-2 text-xs text-emerald-800">{result.targetDir}</p>
                    {result.installed.length > 0 && (
                      <p className="mt-3 text-xs text-emerald-800">
                        {t.claudeInitInstalledList}: {result.installed.join(", ")}
                      </p>
                    )}
                    {result.skipped.length > 0 && (
                      <p className="mt-1 text-xs text-emerald-800">
                        {t.claudeInitSkippedList}:{" "}
                        {result.skipped.map((item) => `${item.slug} (${item.reason})`).join(", ")}
                      </p>
                    )}
                  </div>
                )}
              </div>
            </div>
          </div>
        )}
      </>
    );
  }

  return (
    <>
      <div className="border-b border-slate-200/60 bg-[linear-gradient(135deg,#FFF6E8_0%,#FFFDFC_42%,#F3F7FF_100%)] px-8 py-5">
        <div className="flex flex-col gap-4 rounded-3xl border border-amber-200/80 bg-white/90 px-5 py-5 shadow-[0_18px_40px_rgba(15,23,42,0.06)] backdrop-blur md:flex-row md:items-center md:justify-between">
          <div className="min-w-0">
            <div className="flex items-center gap-2 text-[11px] font-semibold uppercase tracking-[0.22em] text-amber-600">
              <Sparkles className="h-4 w-4" />
              <span>Claude Bootstrap</span>
            </div>
            <h3 className="mt-2 text-lg font-semibold text-slate-900">{t.claudeInitBannerTitle}</h3>
            <p className="mt-1 max-w-3xl text-sm leading-6 text-slate-600">
              {catalog?.targetDirExists
                ? t.claudeInitBannerExistingBody
                : t.claudeInitBannerMissingBody}
            </p>
          </div>
          <div className="flex shrink-0 flex-wrap items-center gap-2">
            <button
              onClick={openModal}
              className="rounded-full bg-slate-900 px-4 py-2 text-sm font-semibold text-white transition-colors hover:bg-slate-800"
            >
              {t.claudeInitBannerAction}
            </button>
            <button
              onClick={handleDismiss}
              className="rounded-full border border-slate-200 px-4 py-2 text-sm font-semibold text-slate-600 transition-colors hover:bg-slate-100"
            >
              {t.claudeInitBannerLater}
            </button>
            <button
              onClick={openDocs}
              className="inline-flex items-center gap-1 rounded-full px-3 py-2 text-sm font-medium text-indigo-600 transition-colors hover:bg-indigo-50"
            >
              <span>{t.claudeInitBannerDocs}</span>
              <ExternalLink className="h-3.5 w-3.5" />
            </button>
          </div>
        </div>
      </div>

      {modalOpen && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-slate-950/45 p-6">
          <div className="max-h-[88vh] w-full max-w-3xl overflow-hidden rounded-3xl border border-slate-200 bg-white shadow-2xl">
            <div className="flex items-center justify-between border-b border-slate-200 px-6 py-5">
              <div>
                <h3 className="text-lg font-semibold text-slate-900">{t.claudeInitModalTitle}</h3>
                <p className="mt-1 text-sm text-slate-500">{t.claudeInitModalSubtitle}</p>
              </div>
              <button
                onClick={() => setModalOpen(false)}
                className="rounded-full p-2 text-slate-400 transition-colors hover:bg-slate-100 hover:text-slate-600"
              >
                <X className="h-4 w-4" />
              </button>
            </div>

            <div className="max-h-[calc(88vh-92px)] overflow-y-auto px-6 py-5">
              {loadingCatalog ? (
                <div className="py-12 text-center text-sm text-slate-500">{t.claudeInitLoading}</div>
              ) : (
                <>
                  <div className="grid gap-4 md:grid-cols-2">
                    <div className="rounded-2xl border border-slate-200 bg-slate-50/70 p-4">
                      <p className="text-[11px] font-semibold uppercase tracking-[0.18em] text-slate-400">
                        {t.claudeInitTargetDir}
                      </p>
                      <p className="mt-2 break-all font-mono text-xs text-slate-700">
                        {catalog?.targetDir || "-"}
                      </p>
                    </div>
                    <div className="rounded-2xl border border-slate-200 bg-slate-50/70 p-4">
                      <p className="text-[11px] font-semibold uppercase tracking-[0.18em] text-slate-400">
                        {t.claudeInitSourceRepo}
                      </p>
                      <p className="mt-2 font-mono text-xs text-slate-700">anthropics/skills@main</p>
                    </div>
                  </div>

                  <div className="mt-4 space-y-3">
                    {!catalog?.claudeCliInstalled && (
                      <div className="rounded-2xl border border-amber-200 bg-amber-50 px-4 py-3 text-sm text-amber-900">
                        {t.claudeInitCliMissingNotice}
                      </div>
                    )}
                    {catalog?.targetDirExists ? (
                      <div className="rounded-2xl border border-emerald-200 bg-emerald-50 px-4 py-3 text-sm text-emerald-900">
                        {t.claudeInitTargetReadyNotice}
                      </div>
                    ) : (
                      <div
                        className={cn(
                          "rounded-2xl px-4 py-3 text-sm",
                          catalog?.canCreateTargetDir
                            ? "border border-blue-200 bg-blue-50 text-blue-900"
                            : "border border-red-200 bg-red-50 text-red-900"
                        )}
                      >
                        {catalog?.canCreateTargetDir
                          ? t.claudeInitTargetMissingNotice
                          : t.claudeInitCannotCreateNotice}
                      </div>
                    )}
                    {(catalog?.existingSkillSlugs.length ?? 0) > 0 && (
                      <div className="rounded-2xl border border-slate-200 bg-slate-50 px-4 py-3 text-sm text-slate-700">
                        {t.claudeInitExistingSkillsNotice(catalog?.existingSkillSlugs.length ?? 0)}
                        <div className="mt-2 flex flex-wrap gap-2">
                          {(catalog?.existingSkillSlugs ?? []).map((slug) => (
                            <span
                              key={slug}
                              className="rounded-full bg-white px-2 py-1 text-[11px] font-medium text-slate-600"
                            >
                              {slug}
                            </span>
                          ))}
                        </div>
                      </div>
                    )}
                  </div>

                  <div className="mt-6">
                    <div className="mb-3 flex items-center gap-2">
                      <FolderPlus className="h-4 w-4 text-indigo-600" />
                      <h4 className="text-sm font-semibold text-slate-900">{t.claudeInitSelectionHint}</h4>
                    </div>

                    <div className="rounded-3xl border border-slate-200">
                      <div className="border-b border-slate-200 px-4 py-3">
                        <p className="text-sm font-semibold text-slate-800">{t.claudeInitRecommended}</p>
                      </div>
                      <div className="space-y-3 px-4 py-4">
                        {catalog?.recommendedSkills.map((skill) => {
                          const exists = catalog.existingSkillSlugs.includes(skill.slug);
                          return (
                            <label
                              key={skill.slug}
                              className="flex cursor-pointer items-start gap-3 rounded-2xl border border-slate-200 bg-slate-50/50 px-4 py-3"
                            >
                              <input
                                type="checkbox"
                                checked={selectedSlugs.includes(skill.slug)}
                                onChange={() => toggleSkill(skill.slug)}
                                className="mt-1 h-4 w-4 rounded border-slate-300 text-indigo-600 focus:ring-indigo-500"
                              />
                              <div className="min-w-0 flex-1">
                                <div className="flex items-center gap-2">
                                  <span className="text-sm font-semibold text-slate-800">{skill.name}</span>
                                  {exists && (
                                    <span className="rounded-full bg-amber-100 px-2 py-0.5 text-[10px] font-semibold text-amber-700">
                                      {t.claudeInitExistingSkillTag}
                                    </span>
                                  )}
                                </div>
                                <p className="mt-1 text-sm text-slate-500">{skill.description}</p>
                              </div>
                            </label>
                          );
                        })}
                      </div>
                    </div>

                    <div className="mt-4 rounded-3xl border border-slate-200">
                      <div className="border-b border-slate-200 px-4 py-3">
                        <p className="text-sm font-semibold text-slate-800">{t.claudeInitOptional}</p>
                      </div>
                      <div className="space-y-3 px-4 py-4">
                        {catalog?.optionalSkills.map((skill) => {
                          const exists = catalog.existingSkillSlugs.includes(skill.slug);
                          return (
                            <label
                              key={skill.slug}
                              className="flex cursor-pointer items-start gap-3 rounded-2xl border border-slate-200 bg-white px-4 py-3"
                            >
                              <input
                                type="checkbox"
                                checked={selectedSlugs.includes(skill.slug)}
                                onChange={() => toggleSkill(skill.slug)}
                                className="mt-1 h-4 w-4 rounded border-slate-300 text-indigo-600 focus:ring-indigo-500"
                              />
                              <div className="min-w-0 flex-1">
                                <div className="flex items-center gap-2">
                                  <span className="text-sm font-semibold text-slate-800">{skill.name}</span>
                                  {exists && (
                                    <span className="rounded-full bg-amber-100 px-2 py-0.5 text-[10px] font-semibold text-amber-700">
                                      {t.claudeInitExistingSkillTag}
                                    </span>
                                  )}
                                </div>
                                <p className="mt-1 text-sm text-slate-500">{skill.description}</p>
                              </div>
                            </label>
                          );
                        })}
                      </div>
                    </div>
                  </div>

                  {error && (
                    <div className="mt-4 rounded-2xl border border-red-200 bg-red-50 px-4 py-3 text-sm text-red-700">
                      {error}
                    </div>
                  )}

                  {result && (
                    <div className="mt-4 rounded-2xl border border-emerald-200 bg-emerald-50 px-4 py-3 text-sm text-emerald-900">
                      <div className="flex items-center gap-2 font-semibold">
                        <CheckCircle2 className="h-4 w-4" />
                        <span>{t.claudeInitInstalledSummary(result.installed.length, result.skipped.length)}</span>
                      </div>
                      <p className="mt-2 text-xs text-emerald-800">{result.targetDir}</p>
                      {result.installed.length > 0 && (
                        <p className="mt-2 text-xs text-emerald-800">
                          {t.claudeInitInstalledList}: {result.installed.join(", ")}
                        </p>
                      )}
                      {result.skipped.length > 0 && (
                        <p className="mt-1 text-xs text-emerald-800">
                          {t.claudeInitSkippedList}:{" "}
                          {result.skipped.map((item) => `${item.slug} (${item.reason})`).join(", ")}
                        </p>
                      )}
                    </div>
                  )}

                  <div className="mt-6 flex flex-wrap items-center justify-between gap-3">
                    <button
                      onClick={openDocs}
                      className="inline-flex items-center gap-1 text-sm font-medium text-indigo-600 transition-colors hover:text-indigo-700"
                    >
                      <span>{t.claudeInitOpenDocs}</span>
                      <ExternalLink className="h-3.5 w-3.5" />
                    </button>
                    <div className="flex items-center gap-2">
                      <button
                        onClick={() => setModalOpen(false)}
                        className="rounded-full border border-slate-200 px-4 py-2 text-sm font-semibold text-slate-600 transition-colors hover:bg-slate-100"
                      >
                        {t.claudeInitClose}
                      </button>
                      <button
                        onClick={handleInstall}
                        disabled={installing || loadingCatalog || !catalog?.canCreateTargetDir || selectedSlugs.length === 0}
                        className={cn(
                          "rounded-full px-4 py-2 text-sm font-semibold text-white transition-colors",
                          installing || loadingCatalog || !catalog?.canCreateTargetDir || selectedSlugs.length === 0
                            ? "cursor-not-allowed bg-slate-300"
                            : "bg-slate-900 hover:bg-slate-800"
                        )}
                      >
                        {installing ? t.claudeInitInstallRunning : installButtonLabel}
                      </button>
                    </div>
                  </div>
                </>
              )}
            </div>
          </div>
        </div>
      )}
    </>
  );
};
