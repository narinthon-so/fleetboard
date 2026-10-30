import {
  PencilLine,
  Circle,
  CircleDot,
  Eye,
  CircleCheck,
  CircleSlash,
  Hand,
  Clock,
  GitCommitVertical,
  Lock,
  Unlink,
  Play,
  Pause,
  ShieldAlert,
  CircleOff,
} from 'lucide-react';
import type { Actor, TaskStatus, AgentRuntimeStatus } from '../types/api';
import { getProviderMonogram, formatShortId } from '../lib/format';
import { cn } from '../lib/utils';

interface AgentChipProps {
  actor: Actor;
  className?: string;
}

export function AgentChip({ actor, className }: AgentChipProps) {
  const monogram = getProviderMonogram(actor.provider, actor.kind);
  const shortId = formatShortId(actor.external_id, actor.label);

  return (
    <span
      className={cn(
        'inline-flex items-center gap-1 font-mono text-xs px-1.5 py-0.5 rounded bg-[var(--muted)] text-[var(--muted-foreground)] border border-[var(--border)] select-none',
        className
      )}
    >
      <span className="font-semibold text-[var(--foreground)]">[{monogram}]</span>
      <span>{shortId}</span>
    </span>
  );
}

interface PriorityBadgeProps {
  priority: number;
  className?: string;
}

export function PriorityBadge({ priority, className }: PriorityBadgeProps) {
  let colorClass = 'text-[var(--priority-p2)]';
  let label = 'P2';

  if (priority === 0) {
    colorClass = 'text-[var(--priority-p0)]';
    label = 'P0';
  } else if (priority === 1) {
    colorClass = 'text-[var(--priority-p1)]';
    label = 'P1';
  } else if (priority === 2) {
    colorClass = 'text-[var(--priority-p2)]';
    label = 'P2';
  } else if (priority >= 3) {
    colorClass = 'text-[var(--priority-p3)]';
    label = `P${priority}`;
  }

  return (
    <span
      className={cn(
        'inline-flex items-center font-mono text-[11px] font-semibold px-1 py-0.2 rounded border border-[var(--border)] bg-[var(--muted)]/50',
        colorClass,
        className
      )}
    >
      {label}
    </span>
  );
}

interface StatusBadgeProps {
  status: TaskStatus;
  className?: string;
}

export function StatusBadge({ status, className }: StatusBadgeProps) {
  switch (status) {
    case 'draft':
      return (
        <span className={cn('inline-flex items-center gap-1 text-xs text-[var(--status-draft)]', className)}>
          <PencilLine className="w-3.5 h-3.5" />
          <span>draft</span>
        </span>
      );
    case 'todo':
      return (
        <span className={cn('inline-flex items-center gap-1 text-xs text-[var(--status-todo)]', className)}>
          <Circle className="w-3.5 h-3.5" />
          <span>ready</span>
        </span>
      );
    case 'in_progress':
      return (
        <span className={cn('inline-flex items-center gap-1 text-xs text-[var(--status-progress)]', className)}>
          <CircleDot className="w-3.5 h-3.5" />
          <span>in progress</span>
        </span>
      );
    case 'review':
      return (
        <span className={cn('inline-flex items-center gap-1 text-xs text-[var(--status-review)]', className)}>
          <Eye className="w-3.5 h-3.5" />
          <span>review</span>
        </span>
      );
    case 'done':
      return (
        <span className={cn('inline-flex items-center gap-1 text-xs text-[var(--status-done)]', className)}>
          <CircleCheck className="w-3.5 h-3.5" />
          <span>done</span>
        </span>
      );
    case 'dropped':
      return (
        <span className={cn('inline-flex items-center gap-1 text-xs text-[var(--status-dropped)]', className)}>
          <CircleSlash className="w-3.5 h-3.5" />
          <span>dropped</span>
        </span>
      );
  }
}

export function StatusIcon({ status, className }: { status: TaskStatus; className?: string }) {
  switch (status) {
    case 'draft':
      return <PencilLine className={cn('w-3.5 h-3.5 text-[var(--status-draft)]', className)} />;
    case 'todo':
      return <Circle className={cn('w-3.5 h-3.5 text-[var(--status-todo)]', className)} />;
    case 'in_progress':
      return <CircleDot className={cn('w-3.5 h-3.5 text-[var(--status-progress)]', className)} />;
    case 'review':
      return <Eye className={cn('w-3.5 h-3.5 text-[var(--status-review)]', className)} />;
    case 'done':
      return <CircleCheck className={cn('w-3.5 h-3.5 text-[var(--status-done)]', className)} />;
    case 'dropped':
      return <CircleSlash className={cn('w-3.5 h-3.5 text-[var(--status-dropped)]', className)} />;
  }
}

export function SignalIcon({
  signal,
  label,
  className,
}: {
  signal: 'needs-me' | 'stale' | 'no-evidence' | 'blocked' | 'orphaned';
  label?: string;
  className?: string;
}) {
  switch (signal) {
    case 'needs-me':
      return (
        <span title={label || 'Needs me'} className={cn('inline-flex items-center gap-1 text-[var(--signal-needs-me)]', className)}>
          <Hand className="w-3.5 h-3.5" />
          {label && <span className="text-xs">{label}</span>}
        </span>
      );
    case 'stale':
      return (
        <span title={label || 'Stale claim'} className={cn('inline-flex items-center gap-1 text-[var(--signal-stale)]', className)}>
          <Clock className="w-3.5 h-3.5" />
          {label && <span className="text-xs">{label}</span>}
        </span>
      );
    case 'no-evidence':
      return (
        <span
          title={label || 'No commits'}
          className={cn(
            'inline-flex items-center gap-1',
            className?.includes('text-') ? undefined : 'text-[var(--signal-no-evidence)]',
            className
          )}
        >
          <GitCommitVertical className="w-3.5 h-3.5" />
          <span className="text-xs">no commits</span>
        </span>
      );
    case 'blocked':
      return (
        <span title={label || 'Blocked'} className={cn('inline-flex items-center gap-1 text-[var(--signal-blocked)]', className)}>
          <Lock className="w-3.5 h-3.5" />
          {label && <span className="text-xs">{label}</span>}
        </span>
      );
    case 'orphaned':
      return (
        <span title={label || 'Orphaned'} className={cn('inline-flex items-center gap-1 text-[var(--signal-orphaned)]', className)}>
          <Unlink className="w-3.5 h-3.5" />
          {label && <span className="text-xs">{label}</span>}
        </span>
      );
  }
}

export function RuntimeStatusBadge({
  status,
  pendingPermissions = 0,
  className,
}: {
  status: AgentRuntimeStatus | string;
  pendingPermissions?: number;
  className?: string;
}) {
  const normalized = status.toLowerCase();
  const isRunning = normalized === 'running';
  const isIdle = normalized === 'idle';
  const isPermission = normalized === 'permission' || normalized === 'waiting-for-permission';
  const isGone = normalized === 'gone';

  let colorClass = 'text-[var(--muted-foreground)] border-[var(--border)] bg-[var(--muted)]/30';
  let dotColorClass = 'bg-[var(--muted-foreground)]';
  let label = normalized;
  let Icon = Circle;

  if (isRunning) {
    colorClass = 'text-[var(--runtime-running)] border-[var(--runtime-running)]/30 bg-[var(--runtime-running)]/10';
    dotColorClass = 'bg-[var(--runtime-running)] animate-pulse';
    label = 'running';
    Icon = Play;
  } else if (isIdle) {
    colorClass = 'text-[var(--runtime-idle)] border-[var(--runtime-idle)]/30 bg-[var(--runtime-idle)]/10';
    dotColorClass = 'bg-[var(--runtime-idle)]';
    label = 'idle';
    Icon = Pause;
  } else if (isPermission) {
    colorClass = 'text-[var(--runtime-permission)] border-[var(--runtime-permission)]/30 bg-[var(--runtime-permission)]/10';
    dotColorClass = 'bg-[var(--runtime-permission)] animate-pulse';
    label = 'permission';
    Icon = ShieldAlert;
  } else if (isGone) {
    colorClass = 'text-[var(--runtime-gone)] border-[var(--runtime-gone)]/30 bg-[var(--runtime-gone)]/10';
    dotColorClass = 'bg-[var(--runtime-gone)]';
    label = 'gone';
    Icon = CircleOff;
  }

  return (
    <span
      data-testid="runtime-status-badge"
      data-runtime-status={normalized}
      title={
        isPermission && pendingPermissions > 0
          ? `Runtime status: ${label} (${pendingPermissions} pending)`
          : `Runtime status: ${label}`
      }
      className={cn(
        'inline-flex items-center gap-1 font-mono text-[10px] px-1 py-0.5 rounded border select-none leading-none shrink-0',
        colorClass,
        className
      )}
    >
      <span className={cn('w-1.5 h-1.5 rounded-full shrink-0', dotColorClass)} />
      <Icon className="w-2.5 h-2.5 shrink-0" />
      <span>{label}</span>
    </span>
  );
}
