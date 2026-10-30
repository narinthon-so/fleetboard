import { X, Keyboard } from 'lucide-react';

interface ShortcutsDialogProps {
  isOpen: boolean;
  onClose: () => void;
}

export function ShortcutsDialog({ isOpen, onClose }: ShortcutsDialogProps) {
  if (!isOpen) return null;

  const shortcuts = [
    { key: '/', desc: 'Focus search bar' },
    { key: 'j', desc: 'Select next card' },
    { key: 'k', desc: 'Select previous card' },
    { key: 'Enter', desc: 'Open selected card details' },
    { key: 'Esc', desc: 'Close open sheet or dialog' },
    { key: 'g n', desc: 'Jump to first Needs me card' },
    { key: '?', desc: 'Show this keyboard shortcuts cheat-sheet' },
  ];

  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-label="Keyboard shortcuts"
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-xs p-4"
    >
      <div className="w-full max-w-sm rounded-lg border border-[var(--border)] bg-[var(--card)] p-4 shadow-xl flex flex-col gap-3">
        <div className="flex items-center justify-between border-b border-[var(--border)] pb-2">
          <div className="flex items-center gap-2">
            <Keyboard className="w-4 h-4 text-[var(--primary)]" />
            <h3 className="text-xs font-semibold text-[var(--foreground)] font-mono">
              Keyboard Shortcuts
            </h3>
          </div>
          <button
            type="button"
            onClick={onClose}
            aria-label="Close shortcuts"
            className="p-1 rounded hover:bg-[var(--accent)] text-[var(--muted-foreground)] hover:text-[var(--foreground)] cursor-pointer"
          >
            <X className="w-4 h-4" />
          </button>
        </div>

        <div className="flex flex-col gap-2 py-1">
          {shortcuts.map((sc, idx) => (
            <div key={idx} className="flex items-center justify-between text-xs font-mono">
              <span className="text-[var(--muted-foreground)] font-sans">{sc.desc}</span>
              <kbd className="px-1.5 py-0.5 rounded bg-[var(--muted)] border border-[var(--border)] text-[var(--foreground)] font-semibold text-[11px]">
                {sc.key}
              </kbd>
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}
