import { useState } from 'react';
import { useQuery } from '@tanstack/react-query';
import { getTaskDetail } from '../lib/api';
import { AgentChip, PriorityBadge, StatusBadge, SignalIcon } from './atoms';
import { formatDuration } from '../lib/format';
import { X, CheckSquare, Square, GitBranch, Shield, Clock, FileText, History, List } from 'lucide-react';
import { cn } from '../lib/utils';

interface CardDetailProps {
  taskId: string;
  onClose: () => void;
  onNavigateTask?: (id: string) => void;
}

export function CardDetail({ taskId, onClose, onNavigateTask }: CardDetailProps) {
  const [activeTab, setActiveTab] = useState<'overview' | 'plan' | 'notes' | 'evidence' | 'history'>('overview');

  const { data: task, isLoading, error } = useQuery({
    queryKey: ['task', taskId],
    queryFn: () => getTaskDetail(taskId),
  });

  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-label={`Task detail for ${taskId}`}
      className="fixed inset-y-0 right-0 z-50 w-full max-w-[560px] bg-[var(--card)] border-l border-[var(--border)] shadow-2xl flex flex-col h-full animate-in slide-in-from-right duration-200"
    >
      {/* Header */}
      <div className="flex items-start justify-between p-4 border-b border-[var(--border)] gap-3 bg-[var(--card)]/90 backdrop-blur">
        <div className="flex flex-col gap-1 min-w-0 flex-1">
          <div className="flex items-center gap-2 flex-wrap">
            <span className="font-mono font-bold text-sm text-[var(--foreground)]">{taskId}</span>
            {task && <PriorityBadge priority={task.priority} />}
            {task && <StatusBadge status={task.status} />}
            {task?.derived.needs_me && <SignalIcon signal="needs-me" label="Needs me" />}
          </div>

          <h2 className="text-sm font-semibold text-[var(--foreground)] leading-snug break-words">
            {task?.title || taskId}
          </h2>

          {task && (
            <div className="flex items-center gap-1.5 text-[11px] font-mono text-[var(--muted-foreground)] flex-wrap">
              <span>{task.project?.name || task.project?.key || 'no project'}</span>
              {task.ticket && (
                <>
                  <span>·</span>
                  <span className="text-[var(--primary)] font-semibold">{task.ticket.key}</span>
                </>
              )}
              <span>·</span>
              <span>created {formatDuration(task.created_at)} ago</span>
            </div>
          )}
        </div>

        <button
          type="button"
          onClick={onClose}
          aria-label="Close task detail"
          className="p-1 rounded hover:bg-[var(--accent)] text-[var(--muted-foreground)] hover:text-[var(--foreground)] transition-colors cursor-pointer shrink-0"
        >
          <X className="w-5 h-5" />
        </button>
      </div>

      {/* Tabs */}
      <div className="flex items-center border-b border-[var(--border)] px-4 bg-[var(--muted)]/30 text-xs font-medium select-none overflow-x-auto">
        <button
          type="button"
          onClick={() => setActiveTab('overview')}
          className={cn(
            'flex items-center gap-1.5 py-2.5 px-3 border-b-2 transition-colors cursor-pointer',
            activeTab === 'overview'
              ? 'border-[var(--primary)] text-[var(--foreground)] font-semibold'
              : 'border-transparent text-[var(--muted-foreground)] hover:text-[var(--foreground)]'
          )}
        >
          <List className="w-3.5 h-3.5" />
          <span>Overview</span>
        </button>

        <button
          type="button"
          onClick={() => setActiveTab('plan')}
          className={cn(
            'flex items-center gap-1.5 py-2.5 px-3 border-b-2 transition-colors cursor-pointer',
            activeTab === 'plan'
              ? 'border-[var(--primary)] text-[var(--foreground)] font-semibold'
              : 'border-transparent text-[var(--muted-foreground)] hover:text-[var(--foreground)]'
          )}
        >
          <FileText className="w-3.5 h-3.5" />
          <span>Plan</span>
        </button>

        <button
          type="button"
          onClick={() => setActiveTab('notes')}
          className={cn(
            'flex items-center gap-1.5 py-2.5 px-3 border-b-2 transition-colors cursor-pointer',
            activeTab === 'notes'
              ? 'border-[var(--primary)] text-[var(--foreground)] font-semibold'
              : 'border-transparent text-[var(--muted-foreground)] hover:text-[var(--foreground)]'
          )}
        >
          <Clock className="w-3.5 h-3.5" />
          <span>Notes {task?.notes ? `(${task.notes.length})` : ''}</span>
        </button>

        <button
          type="button"
          onClick={() => setActiveTab('evidence')}
          className={cn(
            'flex items-center gap-1.5 py-2.5 px-3 border-b-2 transition-colors cursor-pointer',
            activeTab === 'evidence'
              ? 'border-[var(--primary)] text-[var(--foreground)] font-semibold'
              : 'border-transparent text-[var(--muted-foreground)] hover:text-[var(--foreground)]'
          )}
        >
          <GitBranch className="w-3.5 h-3.5" />
          <span>Evidence</span>
        </button>

        <button
          type="button"
          onClick={() => setActiveTab('history')}
          className={cn(
            'flex items-center gap-1.5 py-2.5 px-3 border-b-2 transition-colors cursor-pointer',
            activeTab === 'history'
              ? 'border-[var(--primary)] text-[var(--foreground)] font-semibold'
              : 'border-transparent text-[var(--muted-foreground)] hover:text-[var(--foreground)]'
          )}
        >
          <History className="w-3.5 h-3.5" />
          <span>History</span>
        </button>
      </div>

      {/* Content Area */}
      <div className="flex-1 overflow-y-auto p-4 text-xs">
        {isLoading && (
          <div className="flex flex-col gap-3 animate-pulse">
            <div className="h-4 bg-[var(--muted)] rounded w-1/3" />
            <div className="h-16 bg-[var(--muted)] rounded" />
            <div className="h-4 bg-[var(--muted)] rounded w-1/4" />
            <div className="h-24 bg-[var(--muted)] rounded" />
          </div>
        )}

        {error && (
          <div className="p-3 rounded bg-[var(--destructive)]/10 text-[var(--priority-p0)]">
            Failed to load task details
          </div>
        )}

        {task && activeTab === 'overview' && (
          <div className="flex flex-col gap-4">
            {/* Goal */}
            <div className="flex flex-col gap-1.5">
              <span className="font-mono uppercase text-[11px] text-[var(--muted-foreground)] font-semibold">
                Goal
              </span>
              <div className="p-2.5 rounded bg-[var(--muted)]/40 border border-[var(--border)] text-[var(--foreground)] whitespace-pre-wrap font-sans">
                {task.goal || <span className="text-[var(--muted-foreground)] italic">No goal specified</span>}
              </div>
            </div>

            {/* Acceptance Criteria */}
            <div className="flex flex-col gap-1.5">
              <span className="font-mono uppercase text-[11px] text-[var(--muted-foreground)] font-semibold">
                Acceptance Criteria
              </span>
              <div className="p-2.5 rounded bg-[var(--muted)]/40 border border-[var(--border)] flex flex-col gap-1.5 font-sans">
                {task.acceptance ? (
                  task.acceptance.split('\n').map((line, idx) => {
                    const trimmed = line.trim();
                    const isChecked = trimmed.startsWith('- [x]') || trimmed.startsWith('[x]');
                    const isUnchecked = trimmed.startsWith('- [ ]') || trimmed.startsWith('[ ]');
                    const cleanText = trimmed.replace(/^-\s*\[[ x]\]\s*/, '');

                    return (
                      <div key={idx} className="flex items-start gap-2">
                        {isChecked ? (
                          <CheckSquare className="w-3.5 h-3.5 text-[var(--status-done)] mt-0.5 shrink-0" />
                        ) : isUnchecked ? (
                          <Square className="w-3.5 h-3.5 text-[var(--muted-foreground)] mt-0.5 shrink-0" />
                        ) : (
                          <span className="text-[var(--muted-foreground)]">•</span>
                        )}
                        <span className={cn(isChecked && 'line-through text-[var(--muted-foreground)]')}>
                          {cleanText || line}
                        </span>
                      </div>
                    );
                  })
                ) : (
                  <span className="text-[var(--muted-foreground)] italic">No criteria specified</span>
                )}
              </div>
            </div>

            {/* Claim details */}
            <div className="flex flex-col gap-1.5">
              <span className="font-mono uppercase text-[11px] text-[var(--muted-foreground)] font-semibold">
                Current Claim
              </span>
              {task.claim ? (
                <div className="p-2.5 rounded bg-[var(--muted)]/40 border border-[var(--border)] flex flex-col gap-1.5 font-mono text-[11px]">
                  <div className="flex items-center gap-2">
                    <span className="text-[var(--muted-foreground)]">Claimant:</span>
                    <AgentChip actor={task.claim.actor} />
                  </div>
                  <div className="flex items-center gap-2">
                    <span className="text-[var(--muted-foreground)]">Host:</span>
                    <span>{task.claim.host}</span>
                  </div>
                  <div className="flex items-center gap-2">
                    <span className="text-[var(--muted-foreground)]">Branch:</span>
                    <span className="text-[var(--foreground)] font-semibold">{task.claim.branch || 'main'}</span>
                  </div>
                  <div className="flex items-center gap-2">
                    <span className="text-[var(--muted-foreground)]">Path:</span>
                    <span className="text-[var(--muted-foreground)] truncate">{task.claim.cwd}</span>
                  </div>
                  <div className="flex items-center gap-2">
                    <span className="text-[var(--muted-foreground)]">Claimed:</span>
                    <span>{formatDuration(task.claim.claimed_at)} ago</span>
                  </div>
                </div>
              ) : (
                <div className="p-2.5 rounded bg-[var(--muted)]/20 border border-[var(--border)] text-[var(--muted-foreground)] italic">
                  Unclaimed
                </div>
              )}
            </div>

            {/* Blocked by */}
            {task.blocked_by && task.blocked_by.length > 0 && (
              <div className="flex flex-col gap-1.5">
                <span className="font-mono uppercase text-[11px] text-[var(--muted-foreground)] font-semibold">
                  Blocked By
                </span>
                <div className="flex flex-wrap gap-1.5">
                  {task.blocked_by.map((depId) => (
                    <button
                      key={depId}
                      type="button"
                      onClick={() => onNavigateTask?.(depId)}
                      className="px-2 py-0.5 rounded font-mono text-xs border border-[var(--border)] bg-[var(--muted)] hover:bg-[var(--accent)] text-[var(--foreground)] cursor-pointer"
                    >
                      {depId}
                    </button>
                  ))}
                </div>
              </div>
            )}
          </div>
        )}

        {task && activeTab === 'plan' && (
          <div className="flex flex-col gap-3">
            <div className="flex items-center justify-between">
              <span className="font-mono uppercase text-[11px] text-[var(--muted-foreground)] font-semibold">
                Plan
              </span>
              {task.needs_plan_approval && (
                <span className="font-mono text-[11px] text-[var(--signal-needs-me)] font-semibold">
                  Plan Pending Approval
                </span>
              )}
            </div>
            <div className="p-3 rounded bg-[var(--muted)]/40 border border-[var(--border)] whitespace-pre-wrap font-mono text-xs text-[var(--foreground)] leading-relaxed">
              {task.plan || <span className="text-[var(--muted-foreground)] italic font-sans">No plan submitted yet</span>}
            </div>
          </div>
        )}

        {task && activeTab === 'notes' && (
          <div className="flex flex-col gap-3">
            {task.notes && task.notes.length > 0 ? (
              task.notes.map((note) => (
                <div
                  key={note.id}
                  className="p-3 rounded bg-[var(--muted)]/30 border border-[var(--border)] flex flex-col gap-1.5"
                >
                  <div className="flex items-center justify-between text-[11px] font-mono">
                    <div className="flex items-center gap-1.5">
                      <AgentChip actor={note.actor} />
                      <span className="uppercase text-[var(--muted-foreground)]">[{note.kind}]</span>
                    </div>
                    <span className="text-[var(--muted-foreground)]">{formatDuration(note.created_at)} ago</span>
                  </div>
                  <div className="text-xs text-[var(--foreground)] whitespace-pre-wrap font-sans mt-0.5">
                    {note.body}
                  </div>
                </div>
              ))
            ) : (
              <div className="text-[var(--muted-foreground)] italic text-center py-8">
                No notes on this task
              </div>
            )}
          </div>
        )}

        {task && activeTab === 'evidence' && (
          <div className="flex flex-col gap-3 font-mono text-xs">
            {task.evidence ? (
              <div className="border border-[var(--border)] rounded overflow-hidden">
                <table className="w-full text-left">
                  <tbody>
                    <tr className="border-b border-[var(--border)]">
                      <td className="p-2 text-[var(--muted-foreground)] w-28">Commits</td>
                      <td className="p-2 font-semibold text-[var(--foreground)]">
                        {task.evidence.commits} commits
                      </td>
                    </tr>
                    <tr className="border-b border-[var(--border)]">
                      <td className="p-2 text-[var(--muted-foreground)]">Diffstat</td>
                      <td className="p-2 text-[var(--foreground)]">
                        {task.evidence.diffstat || '—'}
                      </td>
                    </tr>
                    <tr className="border-b border-[var(--border)]">
                      <td className="p-2 text-[var(--muted-foreground)]">Worktree</td>
                      <td className="p-2">
                        {task.evidence.dirty ? (
                          <span className="text-[var(--status-review)]">dirty</span>
                        ) : (
                          <span className="text-[var(--status-done)]">clean</span>
                        )}
                      </td>
                    </tr>
                    <tr className="border-b border-[var(--border)]">
                      <td className="p-2 text-[var(--muted-foreground)]">Rebased</td>
                      <td className="p-2 text-[var(--foreground)]">
                        {task.evidence.rebased ? 'yes' : 'no'}
                      </td>
                    </tr>
                    <tr className="border-b border-[var(--border)]">
                      <td className="p-2 text-[var(--muted-foreground)]">Reachable</td>
                      <td className="p-2 text-[var(--foreground)]">
                        {task.evidence.reachable ? 'live' : 'snapshot'}
                      </td>
                    </tr>
                    {task.evidence.pr && (
                      <tr>
                        <td className="p-2 text-[var(--muted-foreground)]">PR</td>
                        <td className="p-2 text-[var(--status-progress)] font-semibold">
                          #{task.evidence.pr.number} ({task.evidence.pr.state})
                        </td>
                      </tr>
                    )}
                  </tbody>
                </table>
              </div>
            ) : (
              <div className="text-[var(--muted-foreground)] italic text-center py-8 font-sans">
                No evidence computed
              </div>
            )}
          </div>
        )}

        {task && activeTab === 'history' && (
          <div className="flex flex-col gap-2 font-mono text-xs">
            {task.events && task.events.length > 0 ? (
              task.events.map((ev) => (
                <div
                  key={ev.id}
                  className="p-2.5 rounded bg-[var(--muted)]/30 border border-[var(--border)] flex flex-col gap-1"
                >
                  <div className="flex items-center justify-between text-[11px]">
                    <div className="flex items-center gap-1.5">
                      <span className="font-semibold text-[var(--foreground)]">{ev.type}</span>
                      <AgentChip actor={ev.actor} />
                    </div>
                    <span className="text-[var(--muted-foreground)]">{formatDuration(ev.created_at)} ago</span>
                  </div>
                  {ev.proc_chain && (
                    <div className="text-[10px] text-[var(--muted-foreground)] flex items-center gap-1 truncate">
                      <Shield className="w-3 h-3 shrink-0" />
                      <span>{ev.proc_chain}</span>
                      {ev.pid && <span>(pid {ev.pid})</span>}
                    </div>
                  )}
                </div>
              ))
            ) : (
              <div className="text-[var(--muted-foreground)] italic text-center py-8 font-sans">
                No event history
              </div>
            )}
          </div>
        )}
      </div>
    </div>
  );
}
