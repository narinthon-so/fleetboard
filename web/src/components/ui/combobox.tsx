import { useState } from 'react';
import * as Popover from '@radix-ui/react-popover';
import { Check, ChevronsUpDown, Search, Layers } from 'lucide-react';
import { cn } from '../../lib/utils';

export interface ComboboxOption {
  value: string;
  label: string;
  secondary?: string;
}

interface ComboboxProps {
  label: string;
  value: string;
  options: ComboboxOption[];
  placeholder?: string;
  onChange: (value: string) => void;
  className?: string;
}

export function Combobox({
  label,
  value,
  options,
  placeholder = 'all',
  onChange,
  className,
}: ComboboxProps) {
  const [open, setOpen] = useState(false);
  const [search, setSearch] = useState('');

  const selectedOption = options.find((o) => o.value === value);
  const displayLabel = selectedOption ? selectedOption.label : placeholder;

  const filteredOptions = options.filter(
    (o) =>
      o.label.toLowerCase().includes(search.toLowerCase()) ||
      (o.secondary && o.secondary.toLowerCase().includes(search.toLowerCase()))
  );

  return (
    <Popover.Root open={open} onOpenChange={setOpen}>
      <Popover.Trigger asChild>
        <button
          type="button"
          role="combobox"
          aria-expanded={open}
          aria-label={`${label} filter`}
          className={cn(
            'inline-flex items-center justify-between gap-1.5 px-2.5 py-1 text-xs rounded border transition-colors cursor-pointer shrink-0 select-none font-sans',
            'border-[var(--border)] bg-[var(--card)] hover:bg-[var(--accent)] text-[var(--foreground)]',
            'focus:outline-none focus:ring-1 focus:ring-[var(--ring)]',
            className
          )}
        >
          <span className="text-[var(--muted-foreground)]">{label}:</span>
          <span className="font-medium truncate max-w-[140px] text-left">{displayLabel}</span>
          <ChevronsUpDown className="w-3.5 h-3.5 shrink-0 text-[var(--muted-foreground)] opacity-70" />
        </button>
      </Popover.Trigger>

      <Popover.Portal>
        <Popover.Content
          align="start"
          sideOffset={4}
          className="z-50 w-64 rounded-md border border-[var(--border)] bg-[var(--popover)] p-1 text-[var(--popover-foreground)] shadow-md outline-none animate-in fade-in-0 zoom-in-95 font-sans"
        >
          {options.length > 5 && (
            <div className="flex items-center px-2 pb-1 pt-0.5 border-b border-[var(--border)] gap-1.5">
              <Search className="w-3.5 h-3.5 text-[var(--muted-foreground)]" />
              <input
                type="text"
                placeholder={`Search ${label.toLowerCase()}...`}
                value={search}
                onChange={(e) => setSearch(e.target.value)}
                className="w-full bg-transparent text-xs text-[var(--foreground)] outline-none placeholder:text-[var(--muted-foreground)] py-1"
              />
            </div>
          )}

          <div className="max-h-60 overflow-y-auto py-1 flex flex-col gap-0.5 text-xs">
            {/* "All" reset option */}
            <button
              type="button"
              onClick={() => {
                onChange('');
                setOpen(false);
                setSearch('');
              }}
              className={cn(
                'flex items-center justify-between px-2 py-1.5 rounded text-left transition-colors cursor-pointer',
                !value
                  ? 'bg-[var(--accent)] font-semibold text-[var(--foreground)]'
                  : 'hover:bg-[var(--accent)]/50 text-[var(--muted-foreground)]'
              )}
            >
              <span>All {label.toLowerCase()}s</span>
              {!value && <Check className="w-3.5 h-3.5 text-[var(--primary)]" />}
            </button>

            {filteredOptions.map((opt) => {
              const isSelected = opt.value === value;
              return (
                <button
                  key={opt.value}
                  type="button"
                  onClick={() => {
                    onChange(opt.value);
                    setOpen(false);
                    setSearch('');
                  }}
                  className={cn(
                    'flex items-center justify-between px-2 py-1.5 rounded text-left transition-colors cursor-pointer',
                    isSelected
                      ? 'bg-[var(--accent)] font-semibold text-[var(--foreground)]'
                      : 'hover:bg-[var(--accent)]/50 text-[var(--foreground)]'
                  )}
                >
                  <div className="flex flex-col min-w-0 pr-2">
                    <span className="truncate">{opt.label}</span>
                    {opt.secondary && (
                      <span className="text-[11px] text-[var(--muted-foreground)] font-mono truncate">
                        {opt.secondary}
                      </span>
                    )}
                  </div>
                  {isSelected && <Check className="w-3.5 h-3.5 text-[var(--primary)] shrink-0" />}
                </button>
              );
            })}

            {filteredOptions.length === 0 && search && (
              <div className="text-center py-2 text-xs text-[var(--muted-foreground)]">
                No matching {label.toLowerCase()}
              </div>
            )}
          </div>
        </Popover.Content>
      </Popover.Portal>
    </Popover.Root>
  );
}

export interface InstanceItem {
  cwd: string;
  host: string;
  pathTail: string;
  branch: string;
  agent: string;
  projectKey: string;
  projectName: string;
}

export interface InstanceGroup {
  projectKey: string;
  projectName: string;
  instances: InstanceItem[];
}

interface InstanceComboboxProps {
  value: string;
  selectedProject: string;
  groups: InstanceGroup[];
  onSelectInstance: (instanceCwd: string) => void;
  onSelectProject: (projectKey: string) => void;
  className?: string;
}

export function InstanceCombobox({
  value,
  selectedProject,
  groups,
  onSelectInstance,
  onSelectProject,
  className,
}: InstanceComboboxProps) {
  const [open, setOpen] = useState(false);
  const [search, setSearch] = useState('');

  // Find currently selected instance label
  let selectedDisplay = 'all';
  if (value) {
    for (const group of groups) {
      const match = group.instances.find((i) => i.cwd === value);
      if (match) {
        selectedDisplay = `${match.host} · ${match.pathTail} · ${match.branch} · ${match.agent}`;
        break;
      }
    }
  } else if (selectedProject) {
    selectedDisplay = `all in ${selectedProject.split('/').pop()}`;
  }

  return (
    <Popover.Root open={open} onOpenChange={setOpen}>
      <Popover.Trigger asChild>
        <button
          type="button"
          role="combobox"
          aria-expanded={open}
          aria-label="Instance filter"
          className={cn(
            'inline-flex items-center justify-between gap-1.5 px-2.5 py-1 text-xs rounded border transition-colors cursor-pointer shrink-0 select-none font-sans',
            'border-[var(--border)] bg-[var(--card)] hover:bg-[var(--accent)] text-[var(--foreground)]',
            'focus:outline-none focus:ring-1 focus:ring-[var(--ring)]',
            className
          )}
        >
          <span className="text-[var(--muted-foreground)]">Instance:</span>
          <span className="font-medium truncate max-w-[180px] text-left font-mono text-[11px]">
            {selectedDisplay}
          </span>
          <ChevronsUpDown className="w-3.5 h-3.5 shrink-0 text-[var(--muted-foreground)] opacity-70" />
        </button>
      </Popover.Trigger>

      <Popover.Portal>
        <Popover.Content
          align="start"
          sideOffset={4}
          className="z-50 w-80 rounded-md border border-[var(--border)] bg-[var(--popover)] p-1 text-[var(--popover-foreground)] shadow-md outline-none animate-in fade-in-0 zoom-in-95 font-sans"
        >
          <div className="flex items-center px-2 pb-1 pt-0.5 border-b border-[var(--border)] gap-1.5">
            <Search className="w-3.5 h-3.5 text-[var(--muted-foreground)]" />
            <input
              type="text"
              placeholder="Search instances..."
              value={search}
              onChange={(e) => setSearch(e.target.value)}
              className="w-full bg-transparent text-xs text-[var(--foreground)] outline-none placeholder:text-[var(--muted-foreground)] py-1 font-mono"
            />
          </div>

          <div className="max-h-72 overflow-y-auto py-1 flex flex-col gap-1 text-xs">
            {/* Reset option */}
            <button
              type="button"
              onClick={() => {
                onSelectInstance('');
                setOpen(false);
                setSearch('');
              }}
              className={cn(
                'flex items-center justify-between px-2 py-1.5 rounded text-left transition-colors cursor-pointer',
                !value && !selectedProject
                  ? 'bg-[var(--accent)] font-semibold text-[var(--foreground)]'
                  : 'hover:bg-[var(--accent)]/50 text-[var(--muted-foreground)]'
              )}
            >
              <span>All instances across all projects</span>
              {!value && !selectedProject && <Check className="w-3.5 h-3.5 text-[var(--primary)]" />}
            </button>

            {/* Two-level list: project -> instances */}
            {groups.map((group) => {
              const matchingInstances = group.instances.filter((inst) => {
                if (!search) return true;
                const q = search.toLowerCase();
                return (
                  inst.host.toLowerCase().includes(q) ||
                  inst.pathTail.toLowerCase().includes(q) ||
                  inst.branch.toLowerCase().includes(q) ||
                  inst.agent.toLowerCase().includes(q) ||
                  group.projectName.toLowerCase().includes(q) ||
                  group.projectKey.toLowerCase().includes(q)
                );
              });

              if (matchingInstances.length === 0 && search) {
                return null;
              }

              const isProjectSelected = selectedProject === group.projectKey && !value;

              return (
                <div key={group.projectKey} className="flex flex-col gap-0.5 border-t border-[var(--border)]/50 pt-1 mt-0.5">
                  {/* Project Header / Pick Project */}
                  <button
                    type="button"
                    onClick={() => {
                      onSelectProject(group.projectKey);
                      onSelectInstance('');
                      setOpen(false);
                      setSearch('');
                    }}
                    className={cn(
                      'flex items-center justify-between px-2 py-1 rounded text-left font-semibold text-[11px] tracking-tight transition-colors cursor-pointer',
                      isProjectSelected
                        ? 'bg-[var(--accent)] text-[var(--foreground)]'
                        : 'text-[var(--foreground)]/90 hover:bg-[var(--accent)]/40'
                    )}
                  >
                    <div className="flex items-center gap-1.5 truncate">
                      <Layers className="w-3 h-3 text-[var(--primary)] shrink-0" />
                      <span className="truncate">{group.projectName}</span>
                      <span className="text-[10px] text-[var(--muted-foreground)] font-mono">
                        ({matchingInstances.length})
                      </span>
                    </div>
                    {isProjectSelected && <Check className="w-3 h-3 text-[var(--primary)] shrink-0" />}
                  </button>

                  {/* Instances under this project */}
                  <div className="pl-3 flex flex-col gap-0.5">
                    {matchingInstances.map((inst, idx) => {
                      const isSelected = value === inst.cwd;
                      const label = `${inst.host} · ${inst.pathTail} · ${inst.branch} · ${inst.agent}`;

                      return (
                        <button
                          key={idx}
                          type="button"
                          onClick={() => {
                            onSelectInstance(inst.cwd);
                            setOpen(false);
                            setSearch('');
                          }}
                          className={cn(
                            'flex items-center justify-between px-2 py-1 rounded text-left font-mono text-[11px] transition-colors cursor-pointer',
                            isSelected
                              ? 'bg-[var(--accent)] font-semibold text-[var(--foreground)]'
                              : 'hover:bg-[var(--accent)]/50 text-[var(--muted-foreground)] hover:text-[var(--foreground)]'
                          )}
                        >
                          <span className="truncate pr-1">{label}</span>
                          {isSelected && <Check className="w-3 h-3 text-[var(--primary)] shrink-0" />}
                        </button>
                      );
                    })}
                  </div>
                </div>
              );
            })}

            {groups.length === 0 && (
              <div className="text-center py-2 text-xs text-[var(--muted-foreground)]">
                No active instances
              </div>
            )}
          </div>
        </Popover.Content>
      </Popover.Portal>
    </Popover.Root>
  );
}
