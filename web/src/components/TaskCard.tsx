import type { Task } from '../types/api';
import { AgentChip, PriorityBadge, SignalIcon, RuntimeStatusBadge } from './atoms';
import { KanbanCard } from './kanban/kanban';
import { GitBranch, Check, Dot } from 'lucide-react';
import { formatDuration } from '../lib/format';
import { cn } from '../lib/utils';

interface TaskCardProps {
  task: Task;
  isSelected?: boolean;
  onClick?: () => void;
  isReadOnly?: boolean;
}

export function TaskCard({ task, isSelected = false, onClick, isReadOnly = false }: TaskCardProps) {
  const { id, title, priority, status, derived, claim, evidence, project, ticket } = task;
  const isDraft = status === 'draft';
  const isDropped = status === 'dropped';
  const hasNeedsMe = derived.needs_me;

  const runtimeStatus = task.runtime_status || task.runtime?.status;
  const pendingPermissions = task.pending_permissions ?? task.runtime?.pending_permissions ?? 0;
  const isRuntimeGone = runtimeStatus === 'gone';

  // Stale age if claim exists
  const staleText = claim ? formatDuration(claim.last_activity || claim.claimed_at) : null;

  // Repo name: prefer project alias or name, fallback to key tail
  const repoName = project?.alias || project?.name || (project?.key ? project.key.split('/').pop() : 'repo');

  // Tooltip text for hover: full branch and cwd
  const tooltipText = claim
    ? `Branch: ${claim.branch || 'unknown'}\nPath: ${claim.cwd}\nHost: ${claim.host}${runtimeStatus ? `\nRuntime: ${runtimeStatus}${pendingPermissions > 0 ? ` (${pendingPermissions} pending)` : ''}` : ''}`
    : `Project: ${project?.key || 'none'}`;

  return (
    <KanbanCard
      id={id}
      isSelected={isSelected}
      isDraft={isDraft}
      hasNeedsMe={hasNeedsMe}
      onClick={onClick}
      isReadOnly={isReadOnly}
      className="text-xs"
    >
      <div title={tooltipText} className="flex flex-col gap-1 w-full overflow-hidden">
        {/* Line 1: ID, Priority, and right-aligned Flag Icons */}
        <div className="flex items-center justify-between gap-1 leading-tight">
          <div className="flex items-center gap-1.5 font-mono">
            <span className="font-semibold text-[var(--foreground)]">{id}</span>
            <PriorityBadge priority={priority} />
          </div>

          <div className="flex items-center gap-1 text-[11px]">
            {derived.needs_me && <SignalIcon signal="needs-me" />}
            {derived.stale && (
              <span
                className="inline-flex items-center gap-0.5 text-[var(--signal-stale)] font-mono"
                title={isRuntimeGone ? 'Stale: runtime agent gone' : (staleText ? `Stale: ${staleText}` : 'Stale claim')}
              >
                <SignalIcon signal="stale" />
                <span>{isRuntimeGone ? 'gone' : (staleText || 'stale')}</span>
              </span>
            )}
            {derived.blocked && !derived.orphaned && <SignalIcon signal="blocked" />}
            {derived.orphaned && <SignalIcon signal="orphaned" />}
            {derived.no_evidence && <SignalIcon signal="no-evidence" />}
          </div>
        </div>

        {/* Line 2: Title (max 2 lines, clamped) */}
        <div
          className={cn(
            'text-xs font-normal line-clamp-2 leading-snug text-[var(--foreground)] break-words',
            isDropped && 'line-through text-[var(--status-dropped)]'
          )}
        >
          {title}
        </div>

        {/* Line 3: Agent Chip · host · repo · branch OR Unclaimed (project · ticket)
            Polish: Host and repo shrink last, branch shrinks first. Full values in tooltip. */}
        <div className="flex items-center gap-1 text-[11px] font-mono text-[var(--muted-foreground)] leading-none mt-0.5 min-w-0">
          {claim ? (
            <>
              <div className="shrink-0 flex items-center gap-1">
                <AgentChip actor={claim.actor} />
                {runtimeStatus && (
                  <RuntimeStatusBadge
                    status={runtimeStatus}
                    pendingPermissions={pendingPermissions}
                  />
                )}
              </div>
              <span className="shrink-0">·</span>
              <span className="shrink-0 max-w-[90px] truncate" title={claim.host}>{claim.host}</span>
              <span className="shrink-0">·</span>
              <span className="shrink-0 max-w-[110px] truncate" title={repoName}>{repoName}</span>
              <span className="shrink-0">·</span>
              <span className="truncate min-w-0 flex-1" title={claim.branch || 'main'}>{claim.branch || 'main'}</span>
            </>
          ) : (
            <>
              <span className="truncate text-[var(--foreground)]/80" title={repoName}>{repoName}</span>
              {ticket && (
                <>
                  <span className="shrink-0">·</span>
                  <span className="text-[var(--primary)] font-semibold shrink-0" title={ticket.key}>{ticket.key}</span>
                </>
              )}
            </>
          )}
        </div>

        {/* Line 4: Evidence Line (in_progress / review only when claim exists) */}
        {claim && (status === 'in_progress' || status === 'review') && (
          <div className="flex items-center gap-1.5 text-[11px] font-mono text-[var(--muted-foreground)] truncate leading-none mt-0.5">
            {evidence ? (
              evidence.reachable ? (
                evidence.commits > 0 ? (
                  <>
                    <GitBranch className="w-3 h-3 text-[var(--muted-foreground)] shrink-0" />
                    <span>{evidence.commits} commits</span>
                    {evidence.diffstat && (
                      <span className="truncate text-[10px] text-[var(--muted-foreground)]/80">
                        {evidence.diffstat}
                      </span>
                    )}
                    <span className="flex items-center gap-0.5">
                      {evidence.dirty ? (
                        <span className="text-[var(--status-review)] flex items-center">
                          <Dot className="w-3 h-3" /> dirty
                        </span>
                      ) : (
                        <span className="text-[var(--status-done)] flex items-center">
                          <Check className="w-3 h-3" /> clean
                        </span>
                      )}
                    </span>
                    {evidence.pr && (
                      <span className="text-[var(--status-progress)] font-semibold">
                        PR #{evidence.pr.number} ✓
                      </span>
                    )}
                  </>
                ) : (
                  <span
                    className={cn(
                      'flex items-center gap-1',
                      status === 'review'
                        ? 'text-[var(--signal-no-evidence)] font-semibold'
                        : 'text-[var(--muted-foreground)]'
                    )}
                  >
                    <SignalIcon
                      signal="no-evidence"
                      className={status === 'in_progress' ? 'text-[var(--muted-foreground)] font-normal' : undefined}
                    />
                  </span>
                )
              ) : (
                <span className="text-[var(--muted-foreground)]">
                  snapshot {formatDuration(evidence.computed_at)} ago
                </span>
              )
            ) : (
              <span
                className={
                  status === 'review'
                    ? 'text-[var(--signal-no-evidence)] font-semibold'
                    : 'text-[var(--muted-foreground)]'
                }
              >
                no commits
              </span>
            )}
          </div>
        )}
      </div>
    </KanbanCard>
  );
}
