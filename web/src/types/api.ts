export type TaskStatus =
  | 'draft'
  | 'todo'
  | 'in_progress'
  | 'review'
  | 'done'
  | 'dropped';

export type TaskKind = 'code' | 'note';

export interface Project {
  id?: number;
  key: string;
  alias: string | null;
  name: string;
}

export interface Ticket {
  system: string;
  key: string;
  title: string | null;
  url: string | null;
}

export interface Actor {
  id: number;
  kind: 'agent' | 'human';
  provider: string | null;
  external_id: string | null;
  label: string;
  trusted: boolean;
}

export interface Claim {
  id: number;
  actor: Actor;
  host: string;
  cwd: string;
  branch: string | null;
  base_commit: string | null;
  workspace_id: string | null;
  claimed_at: string;
  last_activity: string;
  released_at: string | null;
  release_reason: string | null;
}

export interface Derived {
  ready: boolean;
  blocked: boolean;
  orphaned: boolean;
  plan_pending: boolean;
  stale: boolean;
  no_evidence: boolean;
  needs_me: boolean;
}

export interface EvidencePR {
  number: number;
  state: string;
  url: string;
  statusCheckRollup?: string | null;
}

export interface Evidence {
  commits: number;
  computed_at: string;
  diffstat: string | null;
  dirty: boolean;
  pr: EvidencePR | null;
  reachable: boolean;
  rebased: boolean;
  source: string;
}

export interface Task {
  id: string;
  title: string;
  kind: TaskKind;
  status: TaskStatus;
  priority: number;
  project: Project | null;
  ticket: Ticket | null;
  goal: string | null;
  acceptance: string | null;
  plan: string | null;
  result: string | null;
  needs_plan_approval: boolean;
  plan_approved_at: string | null;
  blocked_by: string[];
  claim: Claim | null;
  derived: Derived;
  evidence: Evidence | null;
  created_by: Actor;
  created_at: string;
  updated_at: string;
}

export interface Instance {
  host: string;
  cwd: string;
  branch: string | null;
  workspace_id: string | null;
  actor: Actor;
  project: Project;
}

export interface BoardData {
  tasks: Task[];
  projects: Project[];
  instances: Instance[];
  actors: Actor[];
}

export interface TaskNote {
  id: number;
  kind: string;
  actor: Actor;
  body: string;
  created_at: string;
}

export interface TaskEvent {
  id: number;
  task_id: string;
  actor: Actor;
  type: string;
  data: Record<string, unknown> | null;
  pid: number | null;
  ppid: number | null;
  proc_chain: string | null;
  created_at: string;
}

export interface TaskDetail extends Task {
  claims: Claim[];
  notes: TaskNote[];
  events: TaskEvent[];
}

export interface ApiResponse<T> {
  v: number;
  ok: boolean;
  data?: T;
  error?: {
    code: string;
    message: string;
  };
}
