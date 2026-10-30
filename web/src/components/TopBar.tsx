import { useRef, useEffect } from 'react';
import type { BoardData } from '../types/api';
import { Search, Moon, Sun } from 'lucide-react';

export type SSEStatus = 'connected' | 'reconnecting' | 'disconnected' | 'daemon_down';

export interface BoardFilters {
  project: string;
  instance: string;
  agent: string;
  host: string;
  search: string;
  showDropped: boolean;
}

interface TopBarProps {
  boardData: BoardData | null;
  filters: BoardFilters;
  onFilterChange: (newFilters: BoardFilters) => void;
  sseStatus: SSEStatus;
  isDark: boolean;
  onToggleTheme: () => void;
}

export function TopBar({
  boardData,
  filters,
  onFilterChange,
  sseStatus,
  isDark,
  onToggleTheme,
}: TopBarProps) {
  const searchInputRef = useRef<HTMLInputElement>(null);

  // Keyboard shortcut '/' to focus search
  useEffect(() => {
    function handleKeyDown(e: KeyboardEvent) {
      if (
        e.key === '/' &&
        document.activeElement?.tagName !== 'INPUT' &&
        document.activeElement?.tagName !== 'TEXTAREA'
      ) {
        e.preventDefault();
        searchInputRef.current?.focus();
      }
    }
    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, []);

  // Compute unique filter options
  const projectOptions = boardData
    ? Array.from(new Set(boardData.tasks.map((t) => t.project?.key).filter(Boolean)))
    : [];

  const hostOptions = boardData
    ? Array.from(
        new Set(
          [
            ...boardData.instances.map((i) => i.host),
            ...boardData.tasks.map((t) => t.claim?.host).filter(Boolean),
          ].filter(Boolean)
        )
      )
    : [];

  const agentOptions = boardData
    ? Array.from(
        new Set(
          boardData.tasks
            .map((t) => t.claim?.actor?.external_id || t.claim?.actor?.label)
            .filter(Boolean)
        )
      )
    : [];

  const instanceOptions = boardData ? boardData.instances : [];

  return (
    <header className="flex items-center justify-between px-4 py-2 border-b border-[var(--border)] bg-[var(--card)]/90 backdrop-blur sticky top-0 z-30 select-none text-xs gap-3">
      {/* Brand */}
      <div className="flex items-center gap-2">
        <span className="font-semibold text-sm tracking-tight text-[var(--foreground)] font-mono">
          fleetboard
        </span>
      </div>

      {/* Filters */}
      <div className="flex items-center gap-2 flex-wrap flex-1 max-w-4xl">
        {/* Project Selector */}
        <select
          aria-label="Filter by project"
          value={filters.project}
          onChange={(e) => onFilterChange({ ...filters, project: e.target.value })}
          className="bg-[var(--card)] border border-[var(--border)] text-[var(--foreground)] rounded px-2 py-1 text-xs focus:ring-1 focus:ring-[var(--ring)] focus:outline-none cursor-pointer"
        >
          <option value="">Project: all</option>
          {projectOptions.map((p) => (
            <option key={p} value={p}>
              {p}
            </option>
          ))}
        </select>

        {/* Instance Selector */}
        <select
          aria-label="Filter by instance"
          value={filters.instance}
          onChange={(e) => onFilterChange({ ...filters, instance: e.target.value })}
          className="bg-[var(--card)] border border-[var(--border)] text-[var(--foreground)] rounded px-2 py-1 text-xs focus:ring-1 focus:ring-[var(--ring)] focus:outline-none cursor-pointer"
        >
          <option value="">Instance: all</option>
          {instanceOptions.map((inst, idx) => {
            const label = `${inst.host} · ${inst.cwd.split('/').pop()} · ${inst.branch || 'main'} · ${inst.actor?.label || 'anon'}`;
            return (
              <option key={idx} value={inst.cwd}>
                {label}
              </option>
            );
          })}
        </select>

        {/* Agent Selector */}
        <select
          aria-label="Filter by agent"
          value={filters.agent}
          onChange={(e) => onFilterChange({ ...filters, agent: e.target.value })}
          className="bg-[var(--card)] border border-[var(--border)] text-[var(--foreground)] rounded px-2 py-1 text-xs focus:ring-1 focus:ring-[var(--ring)] focus:outline-none cursor-pointer"
        >
          <option value="">Agent: all</option>
          {agentOptions.map((a) => (
            <option key={a} value={a}>
              {a}
            </option>
          ))}
        </select>

        {/* Host Selector */}
        <select
          aria-label="Filter by host"
          value={filters.host}
          onChange={(e) => onFilterChange({ ...filters, host: e.target.value })}
          className="bg-[var(--card)] border border-[var(--border)] text-[var(--foreground)] rounded px-2 py-1 text-xs focus:ring-1 focus:ring-[var(--ring)] focus:outline-none cursor-pointer"
        >
          <option value="">Host: all</option>
          {hostOptions.map((h) => (
            <option key={h} value={h}>
              {h}
            </option>
          ))}
        </select>

        {/* Search Input */}
        <div className="relative flex items-center min-w-[160px] max-w-[220px]">
          <Search className="w-3.5 h-3.5 absolute left-2 text-[var(--muted-foreground)] pointer-events-none" />
          <input
            ref={searchInputRef}
            type="text"
            placeholder="/ search"
            value={filters.search}
            onChange={(e) => onFilterChange({ ...filters, search: e.target.value })}
            className="w-full bg-[var(--card)] border border-[var(--border)] text-[var(--foreground)] rounded pl-7 pr-2 py-1 text-xs focus:ring-1 focus:ring-[var(--ring)] focus:outline-none placeholder:text-[var(--muted-foreground)] font-mono"
          />
        </div>

        {/* Dropped toggle */}
        <label className="flex items-center gap-1.5 cursor-pointer text-xs text-[var(--muted-foreground)] select-none">
          <input
            type="checkbox"
            checked={filters.showDropped}
            onChange={(e) => onFilterChange({ ...filters, showDropped: e.target.checked })}
            className="rounded border-[var(--border)] bg-[var(--card)] text-[var(--primary)] focus:ring-[var(--ring)]"
          />
          <span>show dropped</span>
        </label>
      </div>

      {/* Right side: SSE dot & Theme toggle */}
      <div className="flex items-center gap-3 shrink-0">
        {/* SSE Indicator */}
        <div
          title={
            sseStatus === 'connected'
              ? 'Real-time updates active'
              : sseStatus === 'reconnecting'
              ? 'Reconnecting to fleetboard daemon...'
              : 'Disconnected from fleetboard daemon'
          }
          className="flex items-center gap-1 font-mono text-xs select-none"
        >
          {sseStatus === 'connected' && (
            <span className="flex items-center gap-1 text-[var(--status-done)]">
              <span className="w-2 h-2 rounded-full bg-[var(--status-done)] animate-pulse" />
              <span>live</span>
            </span>
          )}
          {sseStatus === 'reconnecting' && (
            <span className="flex items-center gap-1 text-[var(--status-review)]">
              <span className="w-2 h-2 rounded-full bg-[var(--status-review)]" />
              <span>reconnecting</span>
            </span>
          )}
          {(sseStatus === 'disconnected' || sseStatus === 'daemon_down') && (
            <span className="flex items-center gap-1 text-[var(--signal-no-evidence)]">
              <span className="w-2 h-2 rounded-full bg-[var(--signal-no-evidence)]" />
              <span>offline</span>
            </span>
          )}
        </div>

        {/* Theme Toggle */}
        <button
          type="button"
          onClick={onToggleTheme}
          aria-label={isDark ? 'Switch to light mode' : 'Switch to dark mode'}
          className="p-1 rounded hover:bg-[var(--accent)] text-[var(--muted-foreground)] hover:text-[var(--foreground)] transition-colors cursor-pointer"
        >
          {isDark ? <Sun className="w-4 h-4" /> : <Moon className="w-4 h-4" />}
        </button>
      </div>
    </header>
  );
}
