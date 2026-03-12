import React from "react";
import { Sparkles, Terminal } from "lucide-react";
import { getMessages, type Locale } from "../i18n";

interface SkillCardProps {
  locale: Locale;
  skill: {
    id: string;
    slug: string;
    displayName: string;
    description: string;
    installations: any[];
    variants: Array<{
      treeHash: string;
      metadata: {
        version?: string;
      };
    }>;
    hasDiverged: boolean;
  };
}

export const SkillCard: React.FC<SkillCardProps> = ({ skill, locale }) => {
  const t = getMessages(locale);
  const versionHints = skill.variants.map(
    (v) => v.metadata.version?.trim() || v.treeHash.slice(0, 8)
  );
  const versionText =
    new Set(versionHints).size === 1
      ? versionHints[0]
      : `${skill.variants.length} variants`;

  return (
    <div className="relative h-full overflow-hidden rounded-xl border border-slate-200 bg-white p-4 shadow-sm transition-shadow cursor-pointer group hover:shadow-md">
      {skill.hasDiverged && (
        <div className="pointer-events-none absolute -right-8 top-3 rotate-45 bg-amber-500 px-8 py-0.5 text-[10px] font-bold text-white shadow-sm">
          {t.skillDivergedTag}
        </div>
      )}
      <div className={`absolute right-3 ${skill.hasDiverged ? "top-8" : "top-4"} flex items-center space-x-1`}>
        {skill.installations.map((inst, i) => (
          <div key={i} className="h-2 w-2 rounded-full bg-slate-400" title={inst.agentType}></div>
        ))}
      </div>
      <div className="flex justify-between items-start mb-3">
        <div className="bg-indigo-50 p-2 rounded-lg group-hover:bg-indigo-100 transition-colors">
          <Sparkles className="w-5 h-5 text-indigo-600" />
        </div>
      </div>
      <h3 className="font-semibold text-slate-800 mb-1">{skill.displayName || skill.slug}</h3>
      <p className="text-sm text-slate-500 line-clamp-2 min-h-[2.5rem]">{skill.description}</p>
      <div className="mt-4 flex items-center justify-between text-[11px] font-medium text-slate-400 uppercase tracking-wider">
        <span>{versionText}</span>
        <div className="flex items-center space-x-1">
          <Terminal className="w-3 h-3" />
          <span>{t.agentsCount(skill.installations.length)}</span>
        </div>
      </div>
    </div>
  );
};
