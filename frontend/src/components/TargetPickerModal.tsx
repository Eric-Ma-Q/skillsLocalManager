import React, { useMemo, useState } from "react";
import { Check, Layers3, MonitorSmartphone, X } from "lucide-react";
import { formatAgentName, getAgentStatusMeta, type Agent } from "../agentStatus";
import { getMessages, type Locale } from "../i18n";
import type { TargetMode } from "../types";

interface TargetPickerModalProps {
  agents: Agent[];
  confirmLabel: string;
  description: string;
  locale: Locale;
  onClose: () => void;
  onConfirm: (selection: { targetMode: TargetMode; targetAgentType?: string }) => Promise<void> | void;
  title: string;
}

export const TargetPickerModal = ({
  agents,
  confirmLabel,
  description,
  locale,
  onClose,
  onConfirm,
  title,
}: TargetPickerModalProps) => {
  const t = getMessages(locale);
  const [targetMode, setTargetMode] = useState<TargetMode>("all_available");
  const [targetAgentType, setTargetAgentType] = useState<string>(agents[0]?.agentType ?? "");
  const [submitting, setSubmitting] = useState(false);
  const canSubmit = targetMode === "all_available" || Boolean(targetAgentType);
  const sortedAgents = useMemo(
    () => [...agents].sort((left, right) => formatAgentName(left.agentType).localeCompare(formatAgentName(right.agentType))),
    [agents]
  );

  const handleConfirm = async () => {
    if (!canSubmit) return;
    setSubmitting(true);
    try {
      await onConfirm({
        targetMode,
        targetAgentType: targetMode === "single_agent" ? targetAgentType : undefined,
      });
      onClose();
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <div className="fixed inset-0 z-[70] flex items-center justify-center bg-slate-950/45 p-6">
      <div className="flex max-h-[calc(100vh-3rem)] w-full max-w-2xl flex-col overflow-hidden rounded-3xl border border-slate-200 bg-white shadow-2xl">
        <div className="flex items-start justify-between border-b border-slate-100 px-6 py-5">
          <div>
            <h3 className="text-lg font-semibold text-slate-900">{title}</h3>
            <p className="mt-1 text-sm text-slate-500">{description}</p>
          </div>
          <button
            onClick={onClose}
            className="rounded-full p-2 text-slate-400 transition-colors hover:bg-slate-100 hover:text-slate-600"
          >
            <X className="h-4 w-4" />
          </button>
        </div>

        <div className="flex-1 space-y-4 overflow-y-auto px-6 py-5">
          <button
            onClick={() => setTargetMode("all_available")}
            className={`w-full rounded-2xl border px-4 py-4 text-left transition-colors ${
              targetMode === "all_available"
                ? "border-emerald-300 bg-emerald-50"
                : "border-slate-200 bg-slate-50 hover:border-slate-300"
            }`}
          >
            <div className="flex items-center justify-between gap-3">
              <div className="flex items-center gap-3">
                <div className="rounded-xl bg-emerald-100 p-2 text-emerald-700">
                  <Layers3 className="h-4 w-4" />
                </div>
                <div>
                  <div className="text-sm font-semibold text-slate-800">{t.targetAllAvailable}</div>
                  <div className="mt-1 text-xs text-slate-500">{t.targetAllAvailableHint}</div>
                </div>
              </div>
              {targetMode === "all_available" && <Check className="h-4 w-4 text-emerald-600" />}
            </div>
          </button>

          <div
            className={`rounded-2xl border px-4 py-4 transition-colors ${
              targetMode === "single_agent"
                ? "border-indigo-300 bg-indigo-50"
                : "border-slate-200 bg-slate-50"
            }`}
          >
            <button
              onClick={() => setTargetMode("single_agent")}
              className="flex w-full items-center justify-between gap-3 text-left"
            >
              <div className="flex items-center gap-3">
                <div className="rounded-xl bg-indigo-100 p-2 text-indigo-700">
                  <MonitorSmartphone className="h-4 w-4" />
                </div>
                <div>
                  <div className="text-sm font-semibold text-slate-800">{t.targetSingleAgent}</div>
                  <div className="mt-1 text-xs text-slate-500">{t.targetSingleAgentHint}</div>
                </div>
              </div>
              {targetMode === "single_agent" && <Check className="h-4 w-4 text-indigo-600" />}
            </button>

            <div className="mt-4 grid grid-cols-1 gap-2 sm:grid-cols-2">
              {sortedAgents.map((agent) => {
                const status = getAgentStatusMeta(agent, locale);
                const selected = targetAgentType === agent.agentType;
                return (
                  <button
                    key={agent.agentType}
                    onClick={() => {
                      setTargetMode("single_agent");
                      setTargetAgentType(agent.agentType);
                    }}
                    className={`rounded-2xl border px-3 py-3 text-left transition-colors ${
                      selected
                        ? "border-indigo-300 bg-white shadow-sm"
                        : "border-slate-200 bg-white hover:border-slate-300"
                    }`}
                  >
                    <div className="flex items-center justify-between gap-3">
                      <span className="text-sm font-semibold capitalize text-slate-800">
                        {formatAgentName(agent.agentType)}
                      </span>
                      <span className={`rounded-full px-2 py-0.5 text-[10px] font-semibold ${status.badgeClassName}`}>
                        {status.label}
                      </span>
                    </div>
                    <p className="mt-2 text-[11px] text-slate-500">{status.description}</p>
                  </button>
                );
              })}
            </div>
          </div>
        </div>

        <div className="flex items-center justify-end gap-3 border-t border-slate-100 px-6 py-4">
          <button
            onClick={onClose}
            className="rounded-full border border-slate-200 px-4 py-2 text-sm font-medium text-slate-600 transition-colors hover:bg-slate-50"
          >
            {t.closeModal}
          </button>
          <button
            disabled={!canSubmit || submitting}
            onClick={handleConfirm}
            className="rounded-full bg-indigo-600 px-4 py-2 text-sm font-semibold text-white transition-colors hover:bg-indigo-700 disabled:cursor-not-allowed disabled:opacity-60"
          >
            {submitting ? t.working : confirmLabel}
          </button>
        </div>
      </div>
    </div>
  );
};
