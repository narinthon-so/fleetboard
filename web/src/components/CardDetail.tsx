import { useState, useMemo } from 'react';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import {
  getTaskDetail,
  promoteTask,
  approvePlan,
  verifyTask,
  rejectTask,
  releaseTask,
  stealTask,
  dropTask,
  reopenTask,
  addComment,
} from '../lib/api';
import type { Task } from '../types/api';
import { AgentChip, PriorityBadge, StatusBadge, SignalIcon, RuntimeStatusBadge } from './atoms';
import { formatDuration } from '../lib/format';
import { ConfirmDialog, RejectDialog, StealDialog } from './ActionDialogs';
import { toast } from '../lib/toast';
import {
  X,
  CheckSquare,
  Square,
  GitBranch,
  Shield,
  Clock,
  FileText,
  History,
  List,
  RefreshCw,
  Send,
  MoreHorizontal,
  CheckCircle,
  XCircle,
  ArrowRight,
  RotateCcw,
  UserCheck,
  Trash2,
} from 'lucide-react';
import { cn } from '../lib/utils';

interface CardDetailProps {
  taskId: string;
  onClose: () => void;
  onNavigateTask?: (id: string) => void;
  allTasks?: Task[];
  isReadOnly?: boolean;
}

export function CardDetail({
  taskId,
  onClose,
  onNavigateTask,
  allTasks = [],
  isReadOnly = false,
}: CardDetailProps) {
  const queryClient = useQueryClient();
  const [activeTab, setActiveTab] = useState<'overview' | 'plan' | 'notes' | 'evidence' | 'history'>('overview');

  // Comment input state
  const [newComment, setNewComment] = useState('');
  const [isSubmittingComment, setIsSubmittingComment] = useState(false);

  // Dropdown menu state for [⋯]
  const [isMenuOpen, setIsMenuOpen] = useState(false);

  // Action dialog states
  const [isConfirmVerifyOpen, setIsConfirmVerifyOpen] = useState(false);
  const [isRejectOpen, setIsRejectOpen] = useState(false);
  const [isStealOpen, setIsStealOpen] = useState(false);
  const [isDropOpen, setIsDropOpen] = useState(false);
  const [isActionLoading, setIsActionLoading] = useState(false);

  const { data: task, isLoading, error, refetch, isFetching } = useQuery({
    queryKey: ['task', taskId],
    queryFn: () => getTaskDetail(taskId),
  });

  // Calculate tasks that this task blocks
  const blocksTasks = useMemo(() => {
    return allTasks.filter((t) => t.blocked_by && t.blocked_by.includes(taskId));
  }, [allTasks, taskId]);

  // Invalidate queries after action
  async function refreshAll() {
    await Promise.all([
      queryClient.refetchQueries({ queryKey: ['board'] }),
      queryClient.refetchQueries({ queryKey: ['task', taskId] }),
    ]);
  }

  // Action Handlers
  async function handlePromote() {
    if (!task || isReadOnly) return;
    setIsActionLoading(true);
    try {
      await promoteTask(task.id);
      toast(`${task.id} promoted to todo`, 'success');
      await refreshAll();
    } catch (err) {
      toast(err instanceof Error ? err.message : 'Failed to promote task', 'error');
    } finally {
      setIsActionLoading(false);
    }
  }

  async function handleApprove() {
    if (!task || isReadOnly) return;
    setIsActionLoading(true);
    try {
      await approvePlan(task.id);
      toast(`Plan approved for ${task.id}`, 'success');
      await refreshAll();
    } catch (err) {
      toast(err instanceof Error ? err.message : 'Failed to approve plan', 'error');
    } finally {
      setIsActionLoading(false);
    }
  }

  function handleVerifyClick() {
    if (!task || isReadOnly) return;
    const hasEvidence = task.evidence && task.evidence.commits > 0;
    if (task.kind === 'code' && (!hasEvidence || task.derived.no_evidence)) {
      // Must ask for confirmation
      setIsConfirmVerifyOpen(true);
    } else {
      executeVerify(false);
    }
  }

  async function executeVerify(force: boolean) {
    if (!task || isReadOnly) return;
    setIsActionLoading(true);
    setIsConfirmVerifyOpen(false);
    try {
      await verifyTask(task.id, force);
      toast(`${task.id} verified and marked done`, 'success');
      await refreshAll();
    } catch (err) {
      toast(err instanceof Error ? err.message : 'Failed to verify task', 'error');
    } finally {
      setIsActionLoading(false);
    }
  }

  async function executeReject(comment: string) {
    if (!task || isReadOnly) return;
    setIsActionLoading(true);
    setIsRejectOpen(false);
    try {
      await rejectTask(task.id, comment);
      toast(`${task.id} rejected back to in_progress`, 'info');
      await refreshAll();
    } catch (err) {
      toast(err instanceof Error ? err.message : 'Failed to reject task', 'error');
    } finally {
      setIsActionLoading(false);
    }
  }

  async function handleRelease() {
    if (!task || isReadOnly) return;
    setIsActionLoading(true);
    try {
      await releaseTask(task.id);
      toast(`Claim released for ${task.id}`, 'info');
      await refreshAll();
    } catch (err) {
      toast(err instanceof Error ? err.message : 'Failed to release claim', 'error');
    } finally {
      setIsActionLoading(false);
    }
  }

  async function executeSteal(targetActor?: string) {
    if (!task || isReadOnly) return;
    setIsActionLoading(true);
    setIsStealOpen(false);
    try {
      await stealTask(task.id, targetActor);
      toast(`Claim reassigned for ${task.id}`, 'success');
      await refreshAll();
    } catch (err) {
      toast(err instanceof Error ? err.message : 'Failed to reassign claim', 'error');
    } finally {
      setIsActionLoading(false);
    }
  }

  async function executeDrop(reason?: string) {
    if (!task || isReadOnly) return;
    setIsActionLoading(true);
    setIsDropOpen(false);
    try {
      await dropTask(task.id, reason);
      toast(`${task.id} dropped`, 'info');
      await refreshAll();
    } catch (err) {
      toast(err instanceof Error ? err.message : 'Failed to drop task', 'error');
    } finally {
      setIsActionLoading(false);
    }
  }

  async function handleReopen() {
    if (!task || isReadOnly) return;
    setIsActionLoading(true);
    try {
      await reopenTask(task.id);
      toast(`${task.id} reopened to todo`, 'success');
      await refreshAll();
    } catch (err) {
      toast(err instanceof Error ? err.message : 'Failed to reopen task', 'error');
    } finally {
      setIsActionLoading(false);
    }
  }

  async function handleAddComment() {
    if (!task || !newComment.trim() || isReadOnly) return;
    setIsSubmittingComment(true);
    try {
      await addComment(task.id, newComment.trim(), 'comment');
      setNewComment('');
      toast('Comment added', 'success');
      await refreshAll();
    } catch (err) {
      toast(err instanceof Error ? err.message : 'Failed to add comment', 'error');
    } finally {
      setIsSubmittingComment(false);
    }
  }

  const isTerminal = task?.status === 'done' || task?.status === 'dropped';
  const hasPlanPending = Boolean(task?.derived.plan_pending || (task?.needs_plan_approval && !task?.plan_approved_at));

  return (
    <>
      <div
        role="dialog"
        aria-modal="true"
        aria-label={`Task detail for ${taskId}`}
        className="fixed inset-y-0 right-0 z-50 w-full max-w-[560px] bg-[var(--card)] border-l border-[var(--border)] shadow-2xl flex flex-col h-full animate-in slide-in-from-right duration-200"
      >
        {/* Header */}
        <div className="flex flex-col border-b border-[var(--border)] bg-[var(--card)]/90 backdrop-blur">
          <div className="flex items-start justify-between p-4 pb-2 gap-3">
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
            </div>

            <div className="flex items-center gap-1.5 shrink-0">
              {/* Action Buttons depending on state */}
              {task && !isReadOnly && (
                <div className="flex items-center gap-1.5">
                  {/* Draft Actions */}
                  {task.status === 'draft' && (
                    <button
                      type="button"
                      disabled={isActionLoading}
                      onClick={handlePromote}
                      className="px-2.5 py-1 rounded text-xs font-semibold bg-[var(--primary)] text-[var(--primary-foreground)] hover:bg-[var(--primary)]/90 cursor-pointer flex items-center gap-1 shadow-sm transition-colors"
                    >
                      <ArrowRight className="w-3.5 h-3.5" />
                      <span>Promote</span>
                    </button>
                  )}

                  {/* In Progress Actions */}
                  {task.status === 'in_progress' && (
                    <>
                      {hasPlanPending && (
                        <button
                          type="button"
                          disabled={isActionLoading}
                          onClick={handleApprove}
                          className="px-2.5 py-1 rounded text-xs font-semibold bg-[var(--status-progress)] text-black hover:bg-[var(--status-progress)]/90 cursor-pointer flex items-center gap-1 shadow-sm transition-colors"
                        >
                          <UserCheck className="w-3.5 h-3.5" />
                          <span>Approve plan</span>
                        </button>
                      )}
                      <button
                        type="button"
                        disabled={isActionLoading}
                        onClick={handleRelease}
                        className="px-2 py-1 rounded text-xs font-medium border border-[var(--border)] bg-[var(--muted)] hover:bg-[var(--accent)] text-[var(--foreground)] cursor-pointer transition-colors"
                      >
                        Release
                      </button>
                    </>
                  )}

                  {/* Review Actions */}
                  {task.status === 'review' && (
                    <>
                      <button
                        type="button"
                        disabled={isActionLoading}
                        onClick={handleVerifyClick}
                        className="px-2.5 py-1 rounded text-xs font-semibold bg-[var(--status-done)] text-black hover:bg-[var(--status-done)]/90 cursor-pointer flex items-center gap-1 shadow-sm transition-colors"
                      >
                        <CheckCircle className="w-3.5 h-3.5" />
                        <span>Verify</span>
                      </button>
                      <button
                        type="button"
                        disabled={isActionLoading}
                        onClick={() => setIsRejectOpen(true)}
                        className="px-2 py-1 rounded text-xs font-medium border border-[var(--status-review)] text-[var(--status-review)] hover:bg-[var(--status-review)]/10 cursor-pointer flex items-center gap-1 transition-colors"
                      >
                        <XCircle className="w-3.5 h-3.5" />
                        <span>Reject</span>
                      </button>
                    </>
                  )}

                  {/* Reopen Action for Terminal Tasks */}
                  {isTerminal && (
                    <button
                      type="button"
                      disabled={isActionLoading}
                      onClick={handleReopen}
                      className="px-2.5 py-1 rounded text-xs font-semibold bg-[var(--primary)] text-[var(--primary-foreground)] hover:bg-[var(--primary)]/90 cursor-pointer flex items-center gap-1 shadow-sm transition-colors"
                    >
                      <RotateCcw className="w-3.5 h-3.5" />
                      <span>Reopen</span>
                    </button>
                  )}

                  {/* Secondary Actions Menu [⋯] */}
                  {!isTerminal && (
                    <div className="relative">
                      <button
                        type="button"
                        onClick={() => setIsMenuOpen(!isMenuOpen)}
                        aria-label="More actions"
                        className="p-1 rounded border border-[var(--border)] bg-[var(--muted)] hover:bg-[var(--accent)] text-[var(--foreground)] cursor-pointer transition-colors"
                      >
                        <MoreHorizontal className="w-4 h-4" />
                      </button>

                      {isMenuOpen && (
                        <div
                          className="absolute right-0 top-full mt-1 w-36 rounded-md border border-[var(--border)] bg-[var(--card)] shadow-lg py-1 z-50 text-xs font-sans"
                          onMouseLeave={() => setIsMenuOpen(false)}
                        >
                          {task.status === 'in_progress' && (
                            <button
                              type="button"
                              onClick={() => {
                                setIsMenuOpen(false);
                                setIsStealOpen(true);
                              }}
                              className="w-full text-left px-3 py-1.5 hover:bg-[var(--accent)] flex items-center gap-2 text-[var(--foreground)] cursor-pointer"
                            >
                              <UserCheck className="w-3.5 h-3.5" />
                              <span>Steal / Reassign</span>
                            </button>
                          )}
                          <button
                            type="button"
                            onClick={() => {
                              setIsMenuOpen(false);
                              setIsDropOpen(true);
                            }}
                            className="w-full text-left px-3 py-1.5 hover:bg-[var(--accent)] flex items-center gap-2 text-[var(--priority-p0)] cursor-pointer"
                          >
                            <Trash2 className="w-3.5 h-3.5" />
                            <span>Drop task</span>
                          </button>
                        </div>
                      )}
                    </div>
                  )}
                </div>
              )}

              {/* Close Button */}
              <button
                type="button"
                onClick={onClose}
                aria-label="Close task detail"
                className="p-1 rounded hover:bg-[var(--accent)] text-[var(--muted-foreground)] hover:text-[var(--foreground)] transition-colors cursor-pointer shrink-0 ml-1"
              >
                <X className="w-5 h-5" />
              </button>
            </div>
          </div>

          {/* Subline */}
          {task && (
            <div className="flex items-center gap-1.5 px-4 pb-3 text-[11px] font-mono text-[var(--muted-foreground)] flex-wrap">
              <span>{task.project?.alias || task.project?.name || task.project?.key || 'no project'}</span>
              {task.ticket && (
                <>
                  <span>·</span>
                  {task.ticket.url ? (
                    <a
                      href={task.ticket.url}
                      target="_blank"
                      rel="noopener noreferrer"
                      className="text-[var(--primary)] font-semibold hover:underline"
                    >
                      {task.ticket.key} ↗
                    </a>
                  ) : (
                    <span className="text-[var(--primary)] font-semibold">{task.ticket.key}</span>
                  )}
                </>
              )}
              <span>·</span>
              <span>created by {task.created_by.label} {formatDuration(task.created_at)} ago</span>
              {(task.runtime_status || task.runtime?.status) && (
                <>
                  <span>·</span>
                  <RuntimeStatusBadge
                    status={task.runtime_status || task.runtime!.status}
                    pendingPermissions={task.pending_permissions ?? task.runtime?.pending_permissions ?? 0}
                  />
                </>
              )}
            </div>
          )}
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
            {hasPlanPending && (
              <span className="w-2 h-2 rounded-full bg-[var(--signal-needs-me)]" />
            )}
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

              {/* Dependencies (Blocked By & Blocks) */}
              <div className="flex flex-col gap-3">
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

                {blocksTasks.length > 0 && (
                  <div className="flex flex-col gap-1.5">
                    <span className="font-mono uppercase text-[11px] text-[var(--muted-foreground)] font-semibold">
                      Blocks
                    </span>
                    <div className="flex flex-wrap gap-1.5">
                      {blocksTasks.map((bTask) => (
                        <button
                          key={bTask.id}
                          type="button"
                          onClick={() => onNavigateTask?.(bTask.id)}
                          className="px-2 py-0.5 rounded font-mono text-xs border border-[var(--border)] bg-[var(--muted)] hover:bg-[var(--accent)] text-[var(--foreground)] cursor-pointer"
                        >
                          {bTask.id} ({bTask.status})
                        </button>
                      ))}
                    </div>
                  </div>
                )}
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
                    {(task.runtime_status || task.runtime?.status) && (
                      <div className="flex items-center gap-2">
                        <span className="text-[var(--muted-foreground)]">Runtime:</span>
                        <RuntimeStatusBadge
                          status={task.runtime_status || task.runtime!.status}
                          pendingPermissions={task.pending_permissions ?? task.runtime?.pending_permissions ?? 0}
                        />
                      </div>
                    )}
                    {(task.pending_permissions ?? task.runtime?.pending_permissions ?? 0) > 0 && (
                      <div className="flex items-center gap-2">
                        <span className="text-[var(--muted-foreground)]">Permissions:</span>
                        <span className="text-[var(--runtime-permission)] font-semibold">
                          {task.pending_permissions ?? task.runtime?.pending_permissions} pending approval
                        </span>
                      </div>
                    )}
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
            </div>
          )}

          {task && activeTab === 'plan' && (
            <div className="flex flex-col gap-3">
              <div className="flex items-center justify-between">
                <span className="font-mono uppercase text-[11px] text-[var(--muted-foreground)] font-semibold">
                  Plan
                </span>
                {hasPlanPending && !isReadOnly && (
                  <button
                    type="button"
                    disabled={isActionLoading}
                    onClick={handleApprove}
                    className="px-2.5 py-1 rounded text-xs font-semibold bg-[var(--status-progress)] text-black hover:bg-[var(--status-progress)]/90 cursor-pointer flex items-center gap-1 shadow-sm transition-colors"
                  >
                    <UserCheck className="w-3.5 h-3.5" />
                    <span>Approve plan</span>
                  </button>
                )}
              </div>

              {hasPlanPending && (
                <div className="p-2 rounded bg-[var(--status-progress)]/10 border border-[var(--status-progress)]/30 text-[var(--status-progress)] font-mono text-[11px] flex items-center gap-2">
                  <SignalIcon signal="needs-me" />
                  <span>Agent submitted a plan awaiting human approval before proceeding.</span>
                </div>
              )}

              <div className="p-3 rounded bg-[var(--muted)]/40 border border-[var(--border)] whitespace-pre-wrap font-mono text-xs text-[var(--foreground)] leading-relaxed">
                {task.plan || <span className="text-[var(--muted-foreground)] italic font-sans">No plan submitted yet</span>}
              </div>
            </div>
          )}

          {task && activeTab === 'notes' && (
            <div className="flex flex-col gap-4">
              {/* Add Comment Textbox */}
              {!isReadOnly && (
                <div className="flex flex-col gap-2 p-3 rounded-md border border-[var(--border)] bg-[var(--card)]/60">
                  <label className="text-[11px] font-mono font-medium text-[var(--foreground)]">
                    Add Note / Comment
                  </label>
                  <textarea
                    rows={3}
                    value={newComment}
                    onChange={(e) => setNewComment(e.target.value)}
                    placeholder="Write a comment or instruction..."
                    className="w-full px-2.5 py-1.5 rounded border border-[var(--border)] bg-[var(--background)] text-xs text-[var(--foreground)] focus:outline-none focus:ring-1 focus:ring-[var(--ring)] font-sans resize-none"
                    onKeyDown={(e) => {
                      if (e.key === 'Enter' && (e.metaKey || e.ctrlKey)) {
                        e.preventDefault();
                        handleAddComment();
                      }
                    }}
                  />
                  <div className="flex justify-end">
                    <button
                      type="button"
                      disabled={isSubmittingComment || !newComment.trim()}
                      onClick={handleAddComment}
                      className="px-3 py-1 rounded text-xs font-semibold bg-[var(--primary)] text-[var(--primary-foreground)] hover:bg-[var(--primary)]/90 disabled:opacity-50 cursor-pointer flex items-center gap-1.5 transition-colors"
                    >
                      <Send className="w-3 h-3" />
                      <span>{isSubmittingComment ? 'Adding...' : 'Add Comment'}</span>
                    </button>
                  </div>
                </div>
              )}

              {/* Notes List (newest first) */}
              <div className="flex flex-col gap-2.5">
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
                  <div className="text-[var(--muted-foreground)] italic text-center py-6 font-sans">
                    No notes on this task yet
                  </div>
                )}
              </div>
            </div>
          )}

          {task && activeTab === 'evidence' && (
            <div className="flex flex-col gap-3 font-mono text-xs">
              <div className="flex items-center justify-between">
                <span className="font-mono uppercase text-[11px] text-[var(--muted-foreground)] font-semibold">
                  Evidence Table
                </span>
                <button
                  type="button"
                  onClick={() => refetch()}
                  disabled={isFetching}
                  className="px-2 py-1 rounded text-[11px] font-mono border border-[var(--border)] bg-[var(--muted)] hover:bg-[var(--accent)] text-[var(--foreground)] cursor-pointer flex items-center gap-1.5 transition-colors"
                >
                  <RefreshCw className={cn('w-3 h-3', isFetching && 'animate-spin')} />
                  <span>Refresh</span>
                </button>
              </div>

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
                            <span className="text-[var(--status-review)] font-semibold">dirty</span>
                          ) : (
                            <span className="text-[var(--status-done)] font-semibold">clean</span>
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
                        <td className="p-2 text-[var(--muted-foreground)]">Source</td>
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

      {/* Confirmation Dialog for Verify without evidence */}
      <ConfirmDialog
        isOpen={isConfirmVerifyOpen}
        title="Verify without evidence"
        description={`Task ${taskId} has no commit evidence. Verifying it will mark it done without recorded commits. Are you sure you want to verify with --force?`}
        confirmLabel="Verify with force"
        confirmVariant="destructive"
        onConfirm={() => executeVerify(true)}
        onCancel={() => setIsConfirmVerifyOpen(false)}
      />

      {/* Reject Dialog */}
      <RejectDialog
        isOpen={isRejectOpen}
        taskId={taskId}
        onConfirm={executeReject}
        onCancel={() => setIsRejectOpen(false)}
      />

      {/* Steal / Reassign Dialog */}
      <StealDialog
        isOpen={isStealOpen}
        taskId={taskId}
        currentClaimant={task?.claim?.actor.label}
        onConfirm={executeSteal}
        onCancel={() => setIsStealOpen(false)}
      />

      {/* Drop Dialog */}
      <ConfirmDialog
        isOpen={isDropOpen}
        title={`Drop task ${taskId}`}
        description={`Are you sure you want to drop task ${taskId}? Dropped tasks are removed from active board views unless "show dropped" is enabled.`}
        confirmLabel="Drop task"
        confirmVariant="destructive"
        inputLabel="Reason (optional)"
        inputPlaceholder="e.g. Superseded by new requirement"
        onConfirm={executeDrop}
        onCancel={() => setIsDropOpen(false)}
      />
    </>
  );
}
