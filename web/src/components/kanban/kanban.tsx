import React from 'react';
import { cn } from '../../lib/utils';

interface KanbanBoardProps {
  children: React.ReactNode;
  className?: string;
}

export function KanbanBoard({ children, className }: KanbanBoardProps) {
  return (
    <div
      role="region"
      aria-label="Kanban board"
      className={cn('flex flex-row gap-3 overflow-x-auto pb-4 h-full min-h-0', className)}
    >
      {children}
    </div>
  );
}

interface KanbanColumnProps {
  id: string;
  title: string;
  count: number;
  icon?: React.ReactNode;
  isCollapsed?: boolean;
  onToggleCollapse?: () => void;
  children: React.ReactNode;
  className?: string;
}

export function KanbanColumn({
  id,
  title,
  count,
  icon,
  isCollapsed = false,
  onToggleCollapse,
  children,
  className,
}: KanbanColumnProps) {
  if (isCollapsed) {
    return (
      <div
        data-column-id={id}
        onClick={onToggleCollapse}
        className={cn(
          'w-12 shrink-0 flex flex-col items-center py-3 px-1 rounded-md border border-[var(--border)] bg-[var(--card)]/40 hover:bg-[var(--card)] cursor-pointer transition-colors',
          className
        )}
      >
        <div className="flex flex-col items-center gap-2 text-xs font-semibold text-[var(--muted-foreground)]">
          {icon}
          <span className="[writing-mode:vertical-rl] tracking-wider uppercase text-[11px] font-mono">
            {title} ({count})
          </span>
          <span className="text-[10px] text-[var(--muted-foreground)] mt-2">▸</span>
        </div>
      </div>
    );
  }

  return (
    <div
      data-column-id={id}
      className={cn(
        'w-80 shrink-0 flex flex-col rounded-md border border-[var(--border)] bg-[var(--card)]/30 min-h-0 h-full',
        className
      )}
    >
      {/* Column Header */}
      <div className="flex items-center justify-between px-3 py-2 border-b border-[var(--border)] bg-[var(--card)]/80 select-none">
        <div className="flex items-center gap-2">
          {icon}
          <span className="font-semibold text-xs tracking-tight text-[var(--foreground)]">
            {title}
          </span>
          <span className="font-mono text-xs text-[var(--muted-foreground)]">
            {count}
          </span>
        </div>
        {onToggleCollapse && (
          <button
            type="button"
            onClick={onToggleCollapse}
            aria-label={`Collapse ${title} column`}
            className="text-[var(--muted-foreground)] hover:text-[var(--foreground)] text-xs px-1"
          >
            ◂
          </button>
        )}
      </div>

      {/* Column Cards Container */}
      <div className="flex-1 overflow-y-auto p-2 flex flex-col gap-2 min-h-0">
        {count === 0 ? (
          <div className="flex items-center justify-center py-8 text-xs font-mono text-[var(--muted-foreground)]">
            —
          </div>
        ) : (
          children
        )}
      </div>
    </div>
  );
}

interface KanbanCardProps {
  id: string;
  isSelected?: boolean;
  isDraft?: boolean;
  hasNeedsMe?: boolean;
  onClick?: () => void;
  children: React.ReactNode;
  className?: string;
}

export function KanbanCard({
  id,
  isSelected = false,
  isDraft = false,
  hasNeedsMe = false,
  onClick,
  children,
  className,
}: KanbanCardProps) {
  return (
    <div
      data-card-id={id}
      tabIndex={0}
      role="button"
      onClick={onClick}
      onKeyDown={(e) => {
        if (e.key === 'Enter' || e.key === ' ') {
          e.preventDefault();
          onClick?.();
        }
      }}
      className={cn(
        'group relative flex flex-col gap-1 p-2.5 rounded-md border text-left cursor-pointer transition-all duration-150',
        'bg-[var(--card)] hover:bg-[var(--accent)]/50 focus:outline-none focus:ring-1 focus:ring-[var(--ring)]',
        isDraft
          ? 'border-[var(--status-draft)] border-dashed'
          : 'border-[var(--border)]',
        hasNeedsMe && 'border-l-[3px] border-l-[var(--signal-needs-me)]',
        isSelected && 'ring-1 ring-[var(--ring)] border-[var(--primary)]',
        className
      )}
    >
      {children}
    </div>
  );
}
