import { getMessages, type Locale } from "./i18n";

export interface Agent {
  agentType: string;
  isInstalled: boolean;
  skillCount: number;
  configDirectory?: string | null;
  skillsDirectory?: string | null;
  readableSkillsDirectories?: Array<{
    path: string;
    sourceAgentType: string;
    exists: boolean;
  }>;
  configDirectoryExists: boolean;
  skillsDirectoryExists: boolean;
}

export type AgentAvailability = "cli" | "skills" | "config" | "missing";

export interface AgentStatusMeta {
  key: AgentAvailability;
  label: string;
  description: string;
  dotClassName: string;
  badgeClassName: string;
}

const AGENT_NAME_OVERRIDES: Record<string, string> = {
  "gemini-cli": "Gemini Cli",
  "copilot-cli": "Copilot Cli",
  "open-code": "OpenCode",
};

export const formatAgentName = (agentType: string) =>
  AGENT_NAME_OVERRIDES[agentType] ?? agentType.replace(/-/g, " ");

export const isAgentDetected = (agent: Agent) =>
  agent.isInstalled || agent.skillCount > 0 || agent.skillsDirectoryExists || agent.configDirectoryExists;

export const getAgentAvailability = (agent: Agent): AgentAvailability => {
  if (agent.isInstalled) return "cli";
  if (agent.skillCount > 0 || agent.skillsDirectoryExists) return "skills";
  if (agent.configDirectoryExists) return "config";
  return "missing";
};

export const getAgentStatusMeta = (agent: Agent, locale: Locale): AgentStatusMeta => {
  const t = getMessages(locale);

  switch (getAgentAvailability(agent)) {
    case "cli":
      return {
        key: "cli",
        label: t.agentStatusCli,
        description: t.agentStatusCliDescription,
        dotClassName: "bg-emerald-500 shadow-[0_0_8px_rgba(16,185,129,0.35)]",
        badgeClassName: "bg-emerald-50 text-emerald-700 border border-emerald-200/80",
      };
    case "skills":
      return {
        key: "skills",
        label: t.agentStatusSkills,
        description: t.agentStatusSkillsDescription,
        dotClassName: "bg-sky-500 shadow-[0_0_8px_rgba(14,165,233,0.3)]",
        badgeClassName: "bg-sky-50 text-sky-700 border border-sky-200/80",
      };
    case "config":
      return {
        key: "config",
        label: t.agentStatusConfig,
        description: t.agentStatusConfigDescription,
        dotClassName: "bg-amber-400 shadow-[0_0_8px_rgba(251,191,36,0.3)]",
        badgeClassName: "bg-amber-50 text-amber-700 border border-amber-200/80",
      };
    default:
      return {
        key: "missing",
        label: t.agentStatusMissing,
        description: t.agentStatusMissingDescription,
        dotClassName: "bg-slate-300",
        badgeClassName: "bg-slate-100 text-slate-500 border border-slate-200/80",
      };
  }
};
