import { useRef, useEffect, useMemo } from 'react';
import type { BoardData } from '../types/api';
import { agentGroup } from '../lib/format';
import { Combobox, InstanceCombobox, type InstanceGroup } from './ui/combobox';
import { Search, Moon, Sun } from 'lucide-react';
import { BrandLogo } from './BrandLogo';

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

  // Compute Project options
  const projectOptions = useMemo(() => {
    if (!boardData) return [];
    const map = new Map<string, { label: string; secondary?: string }>();
    for (const p of boardData.projects) {
      map.set(p.key, {
        label: p.alias || p.name || p.key.split('/').pop() || p.key,
        secondary: p.key,
      });
    }
    for (const t of boardData.tasks) {
      if (t.project && !map.has(t.project.key)) {
        map.set(t.project.key, {
          label: t.project.alias || t.project.name || t.project.key.split('/').pop() || t.project.key,
          secondary: t.project.key,
        });
      }
    }
    return Array.from(map.entries()).map(([key, data]) => ({
      value: key,
      label: data.label,
      secondary: data.secondary,
    }));
  }, [boardData]);

  // Compute Host options
  const hostOptions = useMemo(() => {
    if (!boardData) return [];
    const set = new Set<string>();
    for (const inst of boardData.instances) {
      if (inst.host) set.add(inst.host);
    }
    for (const t of boardData.tasks) {
      if (t.claim?.host) set.add(t.claim.host);
    }
    return Array.from(set).map((h) => ({ value: h, label: h }));
  }, [boardData]);

  // Compute Agent options: one per provider (human / claude / codex / ...),
  // not one per session. Every agent session registers a new actor, so a
  // per-actor list grows without bound; per-session drill-down is the
  // Instance filter's job.
  const agentOptions = useMemo(() => {
    if (!boardData) return [];
    const counts = new Map<string, number>();
    for (const act of boardData.actors) {
      const key = agentGroup(act);
      if (!counts.has(key)) counts.set(key, 0);
    }
    for (const t of boardData.tasks) {
      if (t.claim?.actor) {
        const key = agentGroup(t.claim.actor);
        counts.set(key, (counts.get(key) ?? 0) + 1);
      }
    }
    const options = Array.from(counts.entries())
      .sort(([a], [b]) => (a === 'human' ? -1 : b === 'human' ? 1 : a.localeCompare(b)))
      .map(([key, claimed]) => ({
        value: key,
        label: key,
        secondary: claimed > 0 ? `${claimed} claimed` : undefined,
      }));
    // An older ?agent=<session id | label> link still filters BoardView by
    // that session; keep it selectable so the selector shows the active
    // filter instead of "all".
    const selected = filters.agent;
    if (selected && !counts.has(selected)) {
      const actor = boardData.actors.find((a) => a.external_id === selected || a.label === selected);
      options.push({
        value: selected,
        label: actor?.label ?? selected,
        secondary: actor ? `${agentGroup(actor)} session` : 'session',
      });
    }
    return options;
  }, [boardData, filters.agent]);

  // Compute Two-Level Instance groups (project -> instances)
  const instanceGroups: InstanceGroup[] = useMemo(() => {
    if (!boardData) return [];

    const projectMap = new Map<
      string,
      {
        projectName: string;
        instancesMap: Map<
          string,
          {
            cwd: string;
            host: string;
            pathTail: string;
            branch: string;
            agent: string;
            projectKey: string;
            projectName: string;
          }
        >;
      }
    >();

    function addInstance(
      projKey: string,
      projName: string,
      cwd: string,
      host: string,
      branch: string | null,
      agentLabel: string
    ) {
      if (!projectMap.has(projKey)) {
        projectMap.set(projKey, {
          projectName: projName,
          instancesMap: new Map(),
        });
      }
      const entry = projectMap.get(projKey)!;
      if (!entry.instancesMap.has(cwd)) {
        const pathTail = cwd.split('/').filter(Boolean).pop() || cwd;
        entry.instancesMap.set(cwd, {
          cwd,
          host,
          pathTail,
          branch: branch || 'main',
          agent: agentLabel,
          projectKey: projKey,
          projectName: projName,
        });
      }
    }

    for (const inst of boardData.instances) {
      const projKey = inst.project?.key || 'unknown';
      const projName = inst.project?.alias || inst.project?.name || projKey.split('/').pop() || projKey;
      addInstance(projKey, projName, inst.cwd, inst.host, inst.branch, inst.actor?.label || 'anon');
    }

    for (const t of boardData.tasks) {
      if (t.claim && t.project) {
        const projKey = t.project.key;
        const projName = t.project.alias || t.project.name || projKey.split('/').pop() || projKey;
        addInstance(
          projKey,
          projName,
          t.claim.cwd,
          t.claim.host,
          t.claim.branch,
          t.claim.actor?.label || 'anon'
        );
      }
    }

    return Array.from(projectMap.entries()).map(([key, data]) => ({
      projectKey: key,
      projectName: data.projectName,
      instances: Array.from(data.instancesMap.values()),
    }));
  }, [boardData]);

  return (
    <header className="flex items-center justify-between px-4 py-2 border-b border-[var(--border)] bg-[var(--card)]/90 backdrop-blur sticky top-0 z-30 select-none text-xs gap-3 flex-wrap lg:flex-nowrap whitespace-nowrap min-h-[44px]">
      {/* Brand */}
      <BrandLogo className="w-[200px]" />

      {/* Keep filters on one scrollable row, below the brand on narrow screens. */}
      <div className="flex items-center gap-2 flex-nowrap order-last w-full min-w-0 overflow-x-auto lg:order-none lg:w-auto">
        {/* Project Combobox */}
        <Combobox
          label="Project"
          value={filters.project}
          options={projectOptions}
          placeholder="all"
          onChange={(val) => onFilterChange({ ...filters, project: val })}
        />

        {/* Instance Combobox: Two-level list project -> instances */}
        <InstanceCombobox
          value={filters.instance}
          selectedProject={filters.project}
          groups={instanceGroups}
          onSelectInstance={(instCwd) => onFilterChange({ ...filters, instance: instCwd })}
          onSelectProject={(projKey) => onFilterChange({ ...filters, project: projKey, instance: '' })}
        />

        {/* Agent Combobox */}
        <Combobox
          label="Agent"
          value={filters.agent}
          options={agentOptions}
          placeholder="all"
          onChange={(val) => onFilterChange({ ...filters, agent: val })}
        />

        {/* Host Combobox */}
        <Combobox
          label="Host"
          value={filters.host}
          options={hostOptions}
          placeholder="all"
          onChange={(val) => onFilterChange({ ...filters, host: val })}
        />

        {/* Search Input */}
        <div className="relative flex items-center w-36 shrink-0">
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
        <label className="flex items-center gap-1.5 cursor-pointer text-xs text-[var(--muted-foreground)] select-none shrink-0 pl-1">
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
      <div className="flex items-center gap-3 shrink-0 ml-auto">
        {/* SSE Indicator */}
        <div
          title={
            sseStatus === 'connected'
              ? 'Real-time updates active'
              : sseStatus === 'reconnecting'
              ? 'Reconnecting to fleetboard daemon...'
              : 'Disconnected from fleetboard daemon'
          }
          className="flex items-center gap-1 font-mono text-xs select-none shrink-0"
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
          className="p-1 rounded hover:bg-[var(--accent)] text-[var(--muted-foreground)] hover:text-[var(--foreground)] transition-colors cursor-pointer shrink-0"
        >
          {isDark ? <Sun className="w-4 h-4" /> : <Moon className="w-4 h-4" />}
        </button>
      </div>
    </header>
  );
}
