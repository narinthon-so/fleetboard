import type { Task } from '../types/api';
import { AgentChip, PriorityBadge, SignalIcon } from './atoms';
import { KanbanCard } from './kanban/kanban';
import { GitBranch, Check, Dot } from 'lucide-react';
import { formatDuration } from '../lib/format';
import { cn } from '../lib/utils';

interface TaskCardProps {
  task: Task;
  isSelected?: boolean;
  onClick?: () => void;
}

export function TaskCard({ task, isSelected = false, onClick }: TaskCardProps) {
  const { id, title, priority, status, derived, claim, evidence, project, ticket } = task;
  const isDraft = status === 'draft';
  const isDropped = status === 'dropped';
  const hasNeedsMe = derived.needs_me;

  // Stale age if claim exists
  const staleText = claim ? formatDuration(claim.last_activity || claim.claimed_at) : null;

  // Repo name: prefer project alias or name, fallback to key tail
  const repoName = project?.alias || project?.name || (project?.key ? project.key.split('/').pop() : 'repo');

  // Tooltip text for hover: full branch and cwd
  const tooltipText = claim
    ? `Branch: ${claim.branch || 'unknown'}\nPath: ${claim.cwd}\nHost: ${claim.host}`
    : `Project: ${project?.key || 'none'}`;

  return (
    <KanbanCard
      id={id}
      isSelected={isSelected}
      isDraft={isDraft}
      hasNeedsMe={hasNeedsMe}
      onClick={onClick}
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
            {derived.stale && staleText && (
              <span className="inline-flex items-center gap-0.5 text-[var(--signal-stale)] font-mono">
                <SignalIcon signal="stale" />
                <span>{staleText}</span>
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

        {/* Line 3: Agent Chip · host · repo · branch OR Unclaimed (project · ticket) */}
        <div className="flex items-center gap-1 text-[11px] font-mono text-[var(--muted-foreground)] truncate leading-none mt-0.5">
          {claim ? (
            <>
              <AgentChip actor={claim.actor} />
              <span>·</span>
              <span className="truncate">{claim.host}</span>
              <span>·</span>
              <span className="truncate">{repoName}</span>
              <span>·</span>
              <span className="truncate">{claim.branch || 'main'}</span>
            </>
          ) : (
            <>
              <span className="truncate text-[var(--foreground)]/80">{repoName}</span>
              {ticket && (
                <>
                  <span>·</span>
                  <span className="text-[var(--primary)] font-semibold">{ticket.key}</span>
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
                  <span className="text-[var(--signal-no-evidence)] font-semibold flex items-center gap-1">
                    <SignalIcon signal="no-evidence" />
                  </span>
                )
              ) : (
                <span className="text-[var(--muted-foreground)]">
                  snapshot {formatDuration(evidence.computed_at)} ago
                </span>
              )
            ) : (
              <span className="text-[var(--signal-no-evidence)]">no commits</span>
            )}
          </div>
        )}
      </div>
    </KanbanCard>
  );
}
