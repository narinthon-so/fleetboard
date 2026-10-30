import { useEffect, useState } from 'react';
import { AlertCircle, AlertTriangle, CheckCircle2, Info, X } from 'lucide-react';
import { cn } from '../lib/utils';
import { toastManager, type ToastMessage } from '../lib/toast';

export function ToastContainer() {
  const [toasts, setToasts] = useState<ToastMessage[]>([]);

  useEffect(() => {
    return toastManager.subscribe((currentToasts) => {
      setToasts(currentToasts);
    });
  }, []);

  if (toasts.length === 0) return null;

  return (
    <div
      role="region"
      aria-label="Notifications"
      className="fixed bottom-4 right-4 z-[100] flex flex-col gap-2 max-w-sm w-full pointer-events-none"
    >
      {toasts.map((t) => (
        <div
          key={t.id}
          role="alert"
          className={cn(
            'pointer-events-auto p-3 rounded-md shadow-xl border bg-[var(--card)] text-[var(--foreground)] text-xs font-mono flex items-start gap-2.5 transition-all duration-200 animate-in slide-in-from-bottom-2',
            t.type === 'error' && 'border-l-4 border-l-[var(--priority-p0)] border-[var(--border)]',
            t.type === 'warning' && 'border-l-4 border-l-[var(--status-review)] border-[var(--border)]',
            t.type === 'info' && 'border-l-4 border-l-[var(--status-progress)] border-[var(--border)]',
            t.type === 'success' && 'border-l-4 border-l-[var(--status-done)] border-[var(--border)]'
          )}
        >
          <div className="shrink-0 mt-0.5">
            {t.type === 'error' && <AlertCircle className="w-4 h-4 text-[var(--priority-p0)]" />}
            {t.type === 'warning' && <AlertTriangle className="w-4 h-4 text-[var(--status-review)]" />}
            {t.type === 'info' && <Info className="w-4 h-4 text-[var(--status-progress)]" />}
            {t.type === 'success' && <CheckCircle2 className="w-4 h-4 text-[var(--status-done)]" />}
          </div>

          <div className="flex-1 font-sans text-xs leading-snug break-words">
            {t.message}
          </div>

          <button
            type="button"
            onClick={() => toastManager.dismiss(t.id)}
            aria-label="Dismiss toast"
            className="shrink-0 p-0.5 rounded text-[var(--muted-foreground)] hover:text-[var(--foreground)] cursor-pointer"
          >
            <X className="w-3.5 h-3.5" />
          </button>
        </div>
      ))}
    </div>
  );
}
