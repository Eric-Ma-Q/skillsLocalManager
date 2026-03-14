export type SkillOriginType =
  | "local-manual"
  | "clawhub-managed"
  | "claude-bootstrap-managed";

export type TargetMode = "all_available" | "single_agent";

export type RegistrySortMode = "updated" | "downloads" | "name";

export type LocalSortMode = "name" | "modified";

export interface ManagedSkillSource {
  provider: SkillOriginType;
  remoteSlug: string;
  sourceRepo?: string | null;
  sourceRef?: string | null;
  installedVersionLabel?: string | null;
  remoteVersionLabel?: string | null;
  registryUrl?: string | null;
  lastSyncedAt?: number | null;
}

export interface SkillVariant {
  id: string;
  uid: string;
  slug: string;
  namespace: string;
  treeHash: string;
  conflictState: "none" | "diverged";
  syncGroupId?: string | null;
  metadata: { name: string; description: string; version?: string | null };
  markdownBody: string;
  installations: any[];
  originType: SkillOriginType;
  originLabel: string;
  originSlug?: string | null;
  managedSource?: ManagedSkillSource | null;
  modifiedAt?: number | null;
  hasUpdate: boolean;
  remoteVersionLabel?: string | null;
  remoteTreeHash?: string | null;
  remoteCommitHash?: string | null;
  localCommitHash?: string | null;
}

export interface SkillGroup {
  id: string;
  slug: string;
  displayName: string;
  description: string;
  installations: any[];
  variants: SkillVariant[];
  hasDiverged: boolean;
}

export interface RegistrySkill {
  slug: string;
  displayName: string;
  summary: string;
  latestVersion?: string | null;
  downloads: number;
  stars: number;
  createdAt?: number | null;
  updatedAt?: number | null;
}

export interface RegistryOwner {
  handle?: string | null;
  displayName?: string | null;
}

export interface RegistrySkillDetail {
  slug: string;
  displayName: string;
  summary: string;
  markdownBody: string;
  latestVersion?: string | null;
  downloads: number;
  stars: number;
  createdAt?: number | null;
  updatedAt?: number | null;
  owner?: RegistryOwner | null;
}

export interface RegistrySkillsResponse {
  items: RegistrySkill[];
  nextCursor?: string | null;
}

export interface ManagedTargetResult {
  targetAgentType: string;
  targetPath: string;
  action: "linked" | "relinked";
}

export interface ManagedSkippedTarget {
  targetAgentType: string;
  reason: string;
}

export interface ManagedSkillActionResponse {
  sourceUid: string;
  sourceSlug: string;
  sourceVersionLabel: string;
  remoteVersionLabel?: string | null;
  updatedSource: boolean;
  alreadyLatest: boolean;
  results: ManagedTargetResult[];
  skipped: ManagedSkippedTarget[];
}
