import { useState, useMemo } from 'react';
import type { BoardData } from '../types/api';
import type { BoardFilters } from './TopBar';
import { KanbanBoard, KanbanColumn } from './kanban/kanban';
import { TaskCard } from './TaskCard';
import { StatusIcon } from './atoms';
import { Terminal } from 'lucide-react';

interface BoardViewProps {
  boardData: BoardData | null;
  filters: BoardFilters;
  isLoading: boolean;
  selectedTaskId: string | null;
  onSelectTask: (id: string | null) => void;
}

export function BoardView({
  boardData,
  filters,
  isLoading,
  selectedTaskId,
  onSelectTask,
}: BoardViewProps) {
  const [isDoneCollapsed, setIsDoneCollapsed] = useState(true);

  // Filter tasks based on the active filters
  const filteredTasks = useMemo(() => {
    if (!boardData) return [];
    return boardData.tasks.filter((task) => {
      // Show dropped toggle
      if (!filters.showDropped && task.status === 'dropped') {
        return false;
      }

      // Project filter
      if (filters.project && task.project?.key !== filters.project) {
        return false;
      }

      // Host filter
      if (filters.host) {
        const matchesHost =
          task.claim?.host === filters.host ||
          boardData.instances.some((i) => i.host === filters.host && i.project.key === task.project?.key);
        if (!matchesHost) return false;
      }

      // Instance filter (matches claim cwd or instance cwd)
      if (filters.instance) {
        const matchesInstance =
          task.claim?.cwd === filters.instance ||
          boardData.instances.some((i) => i.cwd === filters.instance && i.project.key === task.project?.key);
        if (!matchesInstance) return false;
      }

      // Agent filter
      if (filters.agent) {
        const agentMatch =
          task.claim?.actor.external_id === filters.agent ||
          task.claim?.actor.label === filters.agent;
        if (!agentMatch) return false;
      }

      // Search query
      if (filters.search) {
        const query = filters.search.toLowerCase().trim();
        const inId = task.id.toLowerCase().includes(query);
        const inTitle = task.title.toLowerCase().includes(query);
        const inBranch = task.claim?.branch?.toLowerCase().includes(query) ?? false;
        const inAgent = task.claim?.actor?.label.toLowerCase().includes(query) ?? false;
        const inTicket = task.ticket?.key.toLowerCase().includes(query) ?? false;
        if (!inId && !inTitle && !inBranch && !inAgent && !inTicket) {
          return false;
        }
      }

      return true;
    });
  }, [boardData, filters]);

  // Group into columns
  const draftTasks = useMemo(() => filteredTasks.filter((t) => t.status === 'draft'), [filteredTasks]);
  const readyTasks = useMemo(
    () => filteredTasks.filter((t) => t.status === 'todo' && !t.derived.blocked),
    [filteredTasks]
  );
  const blockedTasks = useMemo(
    () => filteredTasks.filter((t) => t.status === 'todo' && t.derived.blocked),
    [filteredTasks]
  );
  const inProgressTasks = useMemo(
    () => filteredTasks.filter((t) => t.status === 'in_progress'),
    [filteredTasks]
  );
  const reviewTasks = useMemo(() => filteredTasks.filter((t) => t.status === 'review'), [filteredTasks]);
  const doneTasks = useMemo(() => filteredTasks.filter((t) => t.status === 'done'), [filteredTasks]);
  const droppedTasks = useMemo(() => filteredTasks.filter((t) => t.status === 'dropped'), [filteredTasks]);

  // Loading skeleton state
  if (isLoading) {
    const columns = ['Draft', 'Ready', 'Blocked', 'In progress', 'Review', 'Done'];
    return (
      <KanbanBoard className="p-4 flex-1">
        {columns.map((colName) => (
          <div
            key={colName}
            className="w-80 shrink-0 flex flex-col rounded-md border border-[var(--border)] bg-[var(--card)]/30 min-h-0 h-full animate-pulse"
          >
            <div className="flex items-center justify-between px-3 py-2 border-b border-[var(--border)] bg-[var(--card)]/80">
              <span className="font-semibold text-xs text-[var(--muted-foreground)]">{colName}</span>
              <span className="w-4 h-3 bg-[var(--muted)] rounded" />
            </div>
            <div className="p-2 flex flex-col gap-2">
              {[1, 2, 3].map((i) => (
                <div key={i} className="h-20 rounded bg-[var(--muted)]/40 border border-[var(--border)]" />
              ))}
            </div>
          </div>
        ))}
      </KanbanBoard>
    );
  }

  // Completely empty board state (no tasks at all in system)
  if (boardData && boardData.tasks.length === 0) {
    return (
      <div className="flex-1 flex items-center justify-center p-8 select-none">
        <div className="flex flex-col items-center text-center max-w-md p-8 rounded-lg border border-[var(--border)] bg-[var(--card)]/60 shadow-sm gap-4">
          <div className="w-12 h-12 rounded-full bg-[var(--muted)] flex items-center justify-center text-[var(--primary)]">
            <Terminal className="w-6 h-6" />
          </div>

          <div className="flex flex-col gap-1">
            <h3 className="font-mono font-semibold text-sm text-[var(--foreground)]">
              Board is empty
            </h3>
            <p className="text-xs text-[var(--muted-foreground)]">
              No tasks have been created yet. Run the CLI to get started:
            </p>
          </div>

          <div className="flex flex-col gap-2 w-full font-mono text-xs text-left bg-[var(--muted)]/50 p-3 rounded border border-[var(--border)] text-[var(--foreground)]">
            <div className="flex items-center gap-2">
              <span className="text-[var(--status-progress)]">$</span>
              <span>fb add "My first task"</span>
            </div>
            <div className="flex items-center gap-2">
              <span className="text-[var(--status-progress)]">$</span>
              <span>fb ready</span>
            </div>
          </div>
        </div>
      </div>
    );
  }

  return (
    <KanbanBoard className="p-4 flex-1">
      {/* Draft Column */}
      <KanbanColumn
        id="draft"
        title="Draft"
        count={draftTasks.length}
        icon={<StatusIcon status="draft" />}
      >
        {draftTasks.map((t) => (
          <TaskCard
            key={t.id}
            task={t}
            isSelected={selectedTaskId === t.id}
            onClick={() => onSelectTask(t.id)}
          />
        ))}
      </KanbanColumn>

      {/* Ready Column */}
      <KanbanColumn
        id="ready"
        title="Ready"
        count={readyTasks.length}
        icon={<StatusIcon status="todo" />}
      >
        {readyTasks.map((t) => (
          <TaskCard
            key={t.id}
            task={t}
            isSelected={selectedTaskId === t.id}
            onClick={() => onSelectTask(t.id)}
          />
        ))}
      </KanbanColumn>

      {/* Blocked Column */}
      <KanbanColumn
        id="blocked"
        title="Blocked"
        count={blockedTasks.length}
        icon={<StatusIcon status="todo" />}
      >
        {blockedTasks.map((t) => (
          <TaskCard
            key={t.id}
            task={t}
            isSelected={selectedTaskId === t.id}
            onClick={() => onSelectTask(t.id)}
          />
        ))}
      </KanbanColumn>

      {/* In progress Column */}
      <KanbanColumn
        id="in_progress"
        title="In progress"
        count={inProgressTasks.length}
        icon={<StatusIcon status="in_progress" />}
      >
        {inProgressTasks.map((t) => (
          <TaskCard
            key={t.id}
            task={t}
            isSelected={selectedTaskId === t.id}
            onClick={() => onSelectTask(t.id)}
          />
        ))}
      </KanbanColumn>

      {/* Review Column */}
      <KanbanColumn
        id="review"
        title="Review"
        count={reviewTasks.length}
        icon={<StatusIcon status="review" />}
      >
        {reviewTasks.map((t) => (
          <TaskCard
            key={t.id}
            task={t}
            isSelected={selectedTaskId === t.id}
            onClick={() => onSelectTask(t.id)}
          />
        ))}
      </KanbanColumn>

      {/* Done Column (collapsed by default) */}
      <KanbanColumn
        id="done"
        title="Done"
        count={doneTasks.length}
        icon={<StatusIcon status="done" />}
        isCollapsed={isDoneCollapsed}
        onToggleCollapse={() => setIsDoneCollapsed(!isDoneCollapsed)}
      >
        {doneTasks.map((t) => (
          <TaskCard
            key={t.id}
            task={t}
            isSelected={selectedTaskId === t.id}
            onClick={() => onSelectTask(t.id)}
          />
        ))}
      </KanbanColumn>

      {/* Dropped Column (only when show dropped filter is active) */}
      {filters.showDropped && (
        <KanbanColumn
          id="dropped"
          title="Dropped"
          count={droppedTasks.length}
          icon={<StatusIcon status="dropped" />}
        >
          {droppedTasks.map((t) => (
            <TaskCard
              key={t.id}
              task={t}
              isSelected={selectedTaskId === t.id}
              onClick={() => onSelectTask(t.id)}
            />
          ))}
        </KanbanColumn>
      )}
    </KanbanBoard>
  );
}
