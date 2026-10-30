import { useState } from 'react';
import { AlertTriangle, AlertCircle, X, ShieldAlert } from 'lucide-react';
import { cn } from '../lib/utils';

export interface ConfirmDialogProps {
  isOpen: boolean;
  title: string;
  description: string;
  confirmLabel?: string;
  confirmVariant?: 'destructive' | 'primary';
  inputLabel?: string;
  inputPlaceholder?: string;
  onConfirm: (inputValue?: string) => void;
  onCancel: () => void;
}

export function ConfirmDialog({
  isOpen,
  title,
  description,
  confirmLabel = 'Confirm',
  confirmVariant = 'primary',
  inputLabel,
  inputPlaceholder,
  onConfirm,
  onCancel,
}: ConfirmDialogProps) {
  if (!isOpen) return null;

  return (
    <ConfirmDialogContent
      title={title}
      description={description}
      confirmLabel={confirmLabel}
      confirmVariant={confirmVariant}
      inputLabel={inputLabel}
      inputPlaceholder={inputPlaceholder}
      onConfirm={onConfirm}
      onCancel={onCancel}
    />
  );
}

function ConfirmDialogContent({
  title,
  description,
  confirmLabel,
  confirmVariant,
  inputLabel,
  inputPlaceholder,
  onConfirm,
  onCancel,
}: Omit<ConfirmDialogProps, 'isOpen'>) {
  const [val, setVal] = useState('');

  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-label={title}
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-xs p-4 animate-in fade-in duration-150"
    >
      <div className="w-full max-w-md rounded-lg border border-[var(--border)] bg-[var(--card)] p-5 shadow-2xl flex flex-col gap-4">
        <div className="flex items-start justify-between gap-3">
          <div className="flex items-center gap-2.5">
            {confirmVariant === 'destructive' ? (
              <AlertCircle className="w-5 h-5 text-[var(--priority-p0)] shrink-0" />
            ) : (
              <AlertTriangle className="w-5 h-5 text-[var(--status-review)] shrink-0" />
            )}
            <h3 className="text-sm font-semibold text-[var(--foreground)] font-mono">{title}</h3>
          </div>
          <button
            type="button"
            onClick={onCancel}
            aria-label="Close dialog"
            className="p-1 rounded text-[var(--muted-foreground)] hover:text-[var(--foreground)] hover:bg-[var(--accent)] cursor-pointer"
          >
            <X className="w-4 h-4" />
          </button>
        </div>

        <p className="text-xs text-[var(--muted-foreground)] leading-relaxed">{description}</p>

        {inputLabel && (
          <div className="flex flex-col gap-1.5">
            <label className="text-[11px] font-mono font-medium text-[var(--foreground)]">
              {inputLabel}
            </label>
            <input
              autoFocus
              type="text"
              value={val}
              onChange={(e) => setVal(e.target.value)}
              placeholder={inputPlaceholder}
              className="w-full px-3 py-1.5 rounded border border-[var(--border)] bg-[var(--background)] text-xs text-[var(--foreground)] focus:outline-none focus:ring-1 focus:ring-[var(--ring)] font-sans"
              onKeyDown={(e) => {
                if (e.key === 'Enter') {
                  e.preventDefault();
                  onConfirm(val.trim() || undefined);
                }
              }}
            />
          </div>
        )}

        <div className="flex items-center justify-end gap-2 pt-2 border-t border-[var(--border)]">
          <button
            type="button"
            onClick={onCancel}
            className="px-3 py-1.5 rounded text-xs font-medium border border-[var(--border)] bg-[var(--muted)] hover:bg-[var(--accent)] text-[var(--foreground)] cursor-pointer transition-colors"
          >
            Cancel
          </button>
          <button
            type="button"
            onClick={() => onConfirm(val.trim() || undefined)}
            className={cn(
              'px-3.5 py-1.5 rounded text-xs font-semibold cursor-pointer transition-colors shadow-sm',
              confirmVariant === 'destructive'
                ? 'bg-[var(--priority-p0)] hover:bg-[var(--priority-p0)]/90 text-[var(--background)]'
                : 'bg-[var(--primary)] hover:bg-[var(--primary)]/90 text-[var(--primary-foreground)]'
            )}
          >
            {confirmLabel}
          </button>
        </div>
      </div>
    </div>
  );
}

export interface RejectDialogProps {
  isOpen: boolean;
  taskId: string;
  onConfirm: (comment: string) => void;
  onCancel: () => void;
}

export function RejectDialog({ isOpen, taskId, onConfirm, onCancel }: RejectDialogProps) {
  if (!isOpen) return null;

  return (
    <RejectDialogContent
      taskId={taskId}
      onConfirm={onConfirm}
      onCancel={onCancel}
    />
  );
}

function RejectDialogContent({
  taskId,
  onConfirm,
  onCancel,
}: Omit<RejectDialogProps, 'isOpen'>) {
  const [comment, setComment] = useState('');
  const [error, setError] = useState<string | null>(null);

  function handleSubmit() {
    if (!comment.trim()) {
      setError('A rejection reason or note is required for the agent.');
      return;
    }
    onConfirm(comment.trim());
  }

  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-label={`Reject task ${taskId}`}
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-xs p-4 animate-in fade-in duration-150"
    >
      <div className="w-full max-w-md rounded-lg border border-[var(--border)] bg-[var(--card)] p-5 shadow-2xl flex flex-col gap-4">
        <div className="flex items-start justify-between gap-3">
          <div className="flex items-center gap-2">
            <AlertCircle className="w-5 h-5 text-[var(--status-review)] shrink-0" />
            <h3 className="text-sm font-semibold text-[var(--foreground)] font-mono">
              Reject {taskId} to In Progress
            </h3>
          </div>
          <button
            type="button"
            onClick={onCancel}
            aria-label="Close dialog"
            className="p-1 rounded text-[var(--muted-foreground)] hover:text-[var(--foreground)] hover:bg-[var(--accent)] cursor-pointer"
          >
            <X className="w-4 h-4" />
          </button>
        </div>

        <p className="text-xs text-[var(--muted-foreground)]">
          Rejection moves the task back to <span className="font-semibold text-[var(--status-progress)]">in_progress</span> while keeping the active agent claim. An explanation note is required.
        </p>

        <div className="flex flex-col gap-1.5">
          <label className="text-[11px] font-mono font-medium text-[var(--foreground)]">
            Rejection Comment / Instructions <span className="text-[var(--priority-p0)]">*</span>
          </label>
          <textarea
            autoFocus
            rows={4}
            value={comment}
            onChange={(e) => {
              setComment(e.target.value);
              if (error) setError(null);
            }}
            placeholder="e.g. Edge cases around token expiration are unhandled in auth.rs..."
            className="w-full px-3 py-2 rounded border border-[var(--border)] bg-[var(--background)] text-xs text-[var(--foreground)] focus:outline-none focus:ring-1 focus:ring-[var(--ring)] font-sans resize-none"
            onKeyDown={(e) => {
              if (e.key === 'Enter' && (e.metaKey || e.ctrlKey)) {
                e.preventDefault();
                handleSubmit();
              }
            }}
          />
          {error && <span className="text-[11px] text-[var(--priority-p0)]">{error}</span>}
        </div>

        <div className="flex items-center justify-end gap-2 pt-2 border-t border-[var(--border)]">
          <button
            type="button"
            onClick={onCancel}
            className="px-3 py-1.5 rounded text-xs font-medium border border-[var(--border)] bg-[var(--muted)] hover:bg-[var(--accent)] text-[var(--foreground)] cursor-pointer transition-colors"
          >
            Cancel
          </button>
          <button
            type="button"
            onClick={handleSubmit}
            className="px-3.5 py-1.5 rounded text-xs font-semibold bg-[var(--status-review)] hover:bg-[var(--status-review)]/90 text-black cursor-pointer transition-colors shadow-sm"
          >
            Reject Task
          </button>
        </div>
      </div>
    </div>
  );
}

export interface StealDialogProps {
  isOpen: boolean;
  taskId: string;
  currentClaimant?: string;
  onConfirm: (targetActor?: string) => void;
  onCancel: () => void;
}

export function StealDialog({
  isOpen,
  taskId,
  currentClaimant,
  onConfirm,
  onCancel,
}: StealDialogProps) {
  if (!isOpen) return null;

  return (
    <StealDialogContent
      taskId={taskId}
      currentClaimant={currentClaimant}
      onConfirm={onConfirm}
      onCancel={onCancel}
    />
  );
}

function StealDialogContent({
  taskId,
  currentClaimant,
  onConfirm,
  onCancel,
}: Omit<StealDialogProps, 'isOpen'>) {
  const [targetActor, setTargetActor] = useState('');

  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-label={`Steal claim for ${taskId}`}
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-xs p-4 animate-in fade-in duration-150"
    >
      <div className="w-full max-w-md rounded-lg border border-[var(--border)] bg-[var(--card)] p-5 shadow-2xl flex flex-col gap-4">
        <div className="flex items-start justify-between gap-3">
          <div className="flex items-center gap-2">
            <ShieldAlert className="w-5 h-5 text-[var(--signal-stale)] shrink-0" />
            <h3 className="text-sm font-semibold text-[var(--foreground)] font-mono">
              Steal / Reassign Claim: {taskId}
            </h3>
          </div>
          <button
            type="button"
            onClick={onCancel}
            aria-label="Close dialog"
            className="p-1 rounded text-[var(--muted-foreground)] hover:text-[var(--foreground)] hover:bg-[var(--accent)] cursor-pointer"
          >
            <X className="w-4 h-4" />
          </button>
        </div>

        <p className="text-xs text-[var(--muted-foreground)]">
          {currentClaimant ? (
            <>
              Currently claimed by <code className="text-[var(--foreground)] font-mono font-semibold">{currentClaimant}</code>.
            </>
          ) : (
            'Take over or reassign this task claim.'
          )}
        </p>

        <div className="flex flex-col gap-1.5">
          <label className="text-[11px] font-mono font-medium text-[var(--foreground)]">
            Reassign To (Actor Spec)
          </label>
          <input
            autoFocus
            type="text"
            value={targetActor}
            onChange={(e) => setTargetActor(e.target.value)}
            placeholder="Leave empty to assign to human, or enter agent ID/label"
            className="w-full px-3 py-1.5 rounded border border-[var(--border)] bg-[var(--background)] text-xs text-[var(--foreground)] focus:outline-none focus:ring-1 focus:ring-[var(--ring)] font-mono"
            onKeyDown={(e) => {
              if (e.key === 'Enter') {
                e.preventDefault();
                onConfirm(targetActor.trim() || undefined);
              }
            }}
          />
          <span className="text-[10px] text-[var(--muted-foreground)]">
            Leave blank to claim as yourself (human operator).
          </span>
        </div>

        <div className="flex items-center justify-end gap-2 pt-2 border-t border-[var(--border)]">
          <button
            type="button"
            onClick={onCancel}
            className="px-3 py-1.5 rounded text-xs font-medium border border-[var(--border)] bg-[var(--muted)] hover:bg-[var(--accent)] text-[var(--foreground)] cursor-pointer transition-colors"
          >
            Cancel
          </button>
          <button
            type="button"
            onClick={() => onConfirm(targetActor.trim() || undefined)}
            className="px-3.5 py-1.5 rounded text-xs font-semibold bg-[var(--primary)] hover:bg-[var(--primary)]/90 text-[var(--primary-foreground)] cursor-pointer transition-colors shadow-sm"
          >
            Reassign
          </button>
        </div>
      </div>
    </div>
  );
}
