import type { Task } from '../types/api';
import { Hand } from 'lucide-react';
import { getNeedsMeReason } from '../lib/format';
import { cn } from '../lib/utils';

interface NeedsMeStripProps {
  tasks: Task[];
  onSelectTask: (id: string) => void;
}

export function NeedsMeStrip({ tasks, onSelectTask }: NeedsMeStripProps) {
  // Only tasks that have needs_me === true
  const needsMeTasks = tasks.filter((t) => t.derived.needs_me);

  return (
    <div
      role="region"
      aria-label="Needs me strip"
      className="flex items-center gap-2 px-4 py-2 border-b border-[var(--border)] bg-[var(--card)]/40 overflow-x-auto whitespace-nowrap min-h-[42px] select-none text-xs"
    >
      <div className="flex items-center gap-1.5 font-semibold text-[var(--signal-needs-me)] shrink-0 pr-2 border-r border-[var(--border)]">
        <Hand className="w-4 h-4" />
        <span className="tracking-wide text-[11px] uppercase">Needs me</span>
        <span className="font-mono text-xs">({needsMeTasks.length})</span>
      </div>

      <div className="flex items-center gap-2 overflow-x-auto flex-1 py-0.5">
        {needsMeTasks.length === 0 ? (
          <span className="text-[var(--muted-foreground)] italic text-xs">
            Nothing needs you
          </span>
        ) : (
          needsMeTasks.map((task) => {
            const reason = getNeedsMeReason(task);
            return (
              <button
                key={task.id}
                type="button"
                onClick={() => onSelectTask(task.id)}
                className={cn(
                  'inline-flex items-center gap-1.5 px-2 py-1 rounded border text-xs font-mono transition-colors shrink-0 cursor-pointer',
                  'border-[var(--signal-needs-me)]/40 bg-[var(--card)] hover:bg-[var(--accent)] text-[var(--foreground)]',
                  'focus:outline-none focus:ring-1 focus:ring-[var(--ring)]'
                )}
              >
                <span className="font-semibold text-[var(--signal-needs-me)]">
                  {task.id}
                </span>
                <span className="text-[var(--muted-foreground)]">·</span>
                <span className="text-[var(--foreground)]/90">{reason}</span>
              </button>
            );
          })
        )}
      </div>
    </div>
  );
}
