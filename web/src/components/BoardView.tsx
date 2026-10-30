import { useState, useMemo } from 'react';
import {
  DndContext,
  DragOverlay,
  PointerSensor,
  MouseSensor,
  TouchSensor,
  useSensor,
  useSensors,
  useDroppable,
  pointerWithin,
  type DragStartEvent,
  type DragEndEvent,
} from '@dnd-kit/core';
import { useQueryClient } from '@tanstack/react-query';
import type { BoardData, Task } from '../types/api';
import type { BoardFilters } from './TopBar';
import { KanbanBoard, KanbanColumn } from './kanban/kanban';
import { TaskCard } from './TaskCard';
import { StatusIcon } from './atoms';
import { Terminal, Trash2 } from 'lucide-react';
import { toast } from '../lib/toast';
import { ConfirmDialog, RejectDialog } from './ActionDialogs';
import { promoteTask, verifyTask, rejectTask, releaseTask, dropTask } from '../lib/api';
import { cn } from '../lib/utils';

interface BoardViewProps {
  boardData: BoardData | null;
  filters: BoardFilters;
  isLoading: boolean;
  selectedTaskId: string | null;
  onSelectTask: (id: string | null) => void;
  isReadOnly?: boolean;
}

function DroppedZone({ isDragging }: { isDragging: boolean }) {
  const { setNodeRef, isOver } = useDroppable({
    id: 'dropped-zone',
  });

  if (!isDragging) return null;

  return (
    <div
      ref={setNodeRef}
      className={cn(
        'w-full py-2.5 px-4 mb-2 rounded-md border-2 border-dashed flex items-center justify-center gap-2 transition-all duration-150 select-none font-mono text-xs',
        isOver
          ? 'border-[var(--status-dropped)] bg-[var(--status-dropped)]/25 text-[var(--foreground)] scale-[1.005]'
          : 'border-[var(--muted-foreground)]/40 bg-[var(--card)]/60 text-[var(--muted-foreground)]'
      )}
    >
      <Trash2 className="w-4 h-4 text-[var(--priority-p0)]" />
      <span className="font-semibold text-[var(--foreground)]">Drop zone</span>
      <span>— drop any card here to drop task (confirm dialog)</span>
    </div>
  );
}

export function BoardView({
  boardData,
  filters,
  isLoading,
  selectedTaskId,
  onSelectTask,
  isReadOnly = false,
}: BoardViewProps) {
  const queryClient = useQueryClient();
  const [isDoneCollapsed, setIsDoneCollapsed] = useState(true);

  // Drag and Drop state
  const [activeTask, setActiveTask] = useState<Task | null>(null);

  // Dialog states for drag-and-drop actions
  const [pendingVerifyTask, setPendingVerifyTask] = useState<Task | null>(null);
  const [pendingRejectTask, setPendingRejectTask] = useState<Task | null>(null);
  const [pendingDropTask, setPendingDropTask] = useState<Task | null>(null);

  const sensors = useSensors(
    useSensor(PointerSensor, {
      activationConstraint: {
        distance: 5,
      },
    }),
    useSensor(MouseSensor, {
      activationConstraint: {
        distance: 5,
      },
    }),
    useSensor(TouchSensor, {
      activationConstraint: {
        delay: 250,
        tolerance: 5,
      },
    })
  );

  function updateBoardTask(updatedTask: Task) {
    queryClient.setQueryData<BoardData>(['board'], (old) => {
      if (!old) return old;
      return {
        ...old,
        tasks: old.tasks.map((t) => (t.id === updatedTask.id ? updatedTask : t)),
      };
    });
    queryClient.invalidateQueries({ queryKey: ['board'] });
    queryClient.invalidateQueries({ queryKey: ['task', updatedTask.id] });
  }

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

  function getColumnForTask(t: Task): string {
    if (t.status === 'draft') return 'draft';
    if (t.status === 'todo') return t.derived.blocked ? 'blocked' : 'ready';
    return t.status;
  }

  function handleDragStart(event: DragStartEvent) {
    if (isReadOnly) return;
    const task = boardData?.tasks.find((t) => t.id === event.active.id);
    console.log('DRAG START:', event.active.id);
    if (task) {
      setActiveTask(task);
    }
  }

  async function handleDragEnd(event: DragEndEvent) {
    const { active, over } = event;
    setActiveTask(null);

    if (!over || isReadOnly) return;

    const taskId = String(active.id);
    const targetId = String(over.id);
    const task = boardData?.tasks.find((t) => t.id === taskId);
    if (!task) return;

    const sourceColumn = getColumnForTask(task);
    const targetColumn = targetId === 'dropped-zone' ? 'dropped' : targetId;
    console.log('DRAG END:', { activeId: active.id, overId: over?.id, sourceColumn, targetColumn });

    if (sourceColumn === targetColumn) {
      // Dropped back in same column: snap back, no toast
      return;
    }

    // 1. Dropped zone / Dropped column
    if (targetColumn === 'dropped') {
      if (task.status === 'dropped') {
        toast(`${task.id} is already dropped`, 'warning');
        return;
      }
      if (task.status === 'done') {
        toast('Done tasks cannot be dropped directly — reopen first if needed', 'warning');
        return;
      }
      setPendingDropTask(task);
      return;
    }

    // 2. Draft -> Ready: promote
    if (sourceColumn === 'draft' && targetColumn === 'ready') {
      try {
        const updated = await promoteTask(task.id);
        updateBoardTask(updated);
        toast(`${task.id} promoted to ready`, 'success');
      } catch (err) {
        toast(err instanceof Error ? err.message : 'Promote failed', 'error');
      }
      return;
    }

    // 3. Review -> Done: verify (opens confirm dialog if no evidence)
    if (sourceColumn === 'review' && targetColumn === 'done') {
      const hasEvidence = task.evidence && task.evidence.commits > 0;
      console.log('CHECK EVIDENCE:', { id: task.id, hasEvidence, commits: task.evidence?.commits, no_evidence: task.derived?.no_evidence });
      if (!hasEvidence || task.derived.no_evidence) {
        console.log('SETTING PENDING VERIFY TASK:', task.id);
        setPendingVerifyTask(task);
      } else {
        try {
          console.log('CALLING VERIFY DIRECTLY:', task.id);
          const updated = await verifyTask(task.id, false);
          updateBoardTask(updated);
          toast(`${task.id} verified and marked done`, 'success');
        } catch (err) {
          toast(err instanceof Error ? err.message : 'Verify failed', 'error');
        }
      }
      return;
    }

    // 4. Review -> In progress: reject (opens comment dialog)
    if (sourceColumn === 'review' && targetColumn === 'in_progress') {
      setPendingRejectTask(task);
      return;
    }

    // 5. In progress (stale) -> Ready: release
    if (sourceColumn === 'in_progress' && targetColumn === 'ready') {
      if (task.derived.stale) {
        try {
          const updated = await releaseTask(task.id);
          updateBoardTask(updated);
          toast(`Claim released for ${task.id}`, 'info');
        } catch (err) {
          toast(err instanceof Error ? err.message : 'Release failed', 'error');
        }
      } else {
        toast(`${task.id} is not stale — only stale claims can be released to Ready`, 'warning');
      }
      return;
    }

    // Invalid drop transitions: snap back and show specific reason toast
    if (sourceColumn === 'draft') {
      if (targetColumn === 'in_progress') {
        toast(`${task.id} has no claim — agents move cards by claiming`, 'warning');
      } else {
        toast(`Cannot move draft cards to ${targetColumn}`, 'warning');
      }
      return;
    }

    if (sourceColumn === 'ready') {
      if (targetColumn === 'in_progress') {
        toast(`${task.id} has no claim — agents move cards by claiming`, 'warning');
      } else {
        toast(`Cannot move ready cards to ${targetColumn}`, 'warning');
      }
      return;
    }

    if (sourceColumn === 'blocked') {
      toast(`${task.id} is blocked by incomplete dependencies (${task.blocked_by.map((dep) => dep.id).join(', ')})`, 'warning');
      return;
    }

    if (sourceColumn === 'in_progress') {
      if (targetColumn === 'review') {
        toast(`Only active agents move cards to review via 'fb done'`, 'warning');
      } else if (targetColumn === 'done') {
        toast(`In progress tasks must reach Review before verification`, 'warning');
      } else {
        toast(`Cannot move in_progress cards to ${targetColumn}`, 'warning');
      }
      return;
    }

    if (sourceColumn === 'review') {
      toast(`Review tasks must be verified to Done or rejected to In progress`, 'warning');
      return;
    }

    if (sourceColumn === 'done' || sourceColumn === 'dropped') {
      toast(`${task.id} is terminal — use Reopen in task detail to reopen`, 'warning');
      return;
    }

    toast(`Invalid transition from ${sourceColumn} to ${targetColumn}`, 'warning');
  }

  // Execution from Dialogs
  async function confirmVerifyWithoutEvidence() {
    if (!pendingVerifyTask) return;
    const task = pendingVerifyTask;
    setPendingVerifyTask(null);
    try {
      const updated = await verifyTask(task.id, true);
      updateBoardTask(updated);
      toast(`${task.id} verified with --force`, 'success');
    } catch (err) {
      toast(err instanceof Error ? err.message : 'Verify failed', 'error');
    }
  }

  async function confirmRejectWithComment(comment: string) {
    if (!pendingRejectTask) return;
    const task = pendingRejectTask;
    setPendingRejectTask(null);
    try {
      const updated = await rejectTask(task.id, comment);
      updateBoardTask(updated);
      toast(`${task.id} rejected back to in_progress`, 'info');
    } catch (err) {
      toast(err instanceof Error ? err.message : 'Reject failed', 'error');
    }
  }

  async function confirmDropWithReason(reason?: string) {
    if (!pendingDropTask) return;
    const task = pendingDropTask;
    setPendingDropTask(null);
    try {
      const updated = await dropTask(task.id, reason);
      updateBoardTask(updated);
      toast(`${task.id} dropped`, 'info');
    } catch (err) {
      toast(err instanceof Error ? err.message : 'Drop failed', 'error');
    }
  }

  // Loading skeleton state
  if (isLoading) {
    const columns = ['Draft', 'Ready', 'Blocked', 'In progress', 'Review'];
    return (
      <KanbanBoard className="p-3 flex-1 w-full max-w-full">
        {columns.map((colName) => (
          <div
            key={colName}
            className="flex-1 min-w-[200px] flex flex-col rounded-md border border-[var(--border)] bg-[var(--card)]/30 min-h-0 h-full animate-pulse"
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
        {/* Collapsed Done column skeleton */}
        <div className="w-12 shrink-0 flex flex-col items-center py-3 px-1 rounded-md border border-[var(--border)] bg-[var(--card)]/40 animate-pulse">
          <div className="w-4 h-4 rounded-full bg-[var(--muted)] mb-2" />
          <div className="w-2 h-12 bg-[var(--muted)] rounded" />
        </div>
      </KanbanBoard>
    );
  }

  // Completely empty board state
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
    <DndContext
      sensors={sensors}
      collisionDetection={pointerWithin}
      onDragStart={handleDragStart}
      onDragEnd={handleDragEnd}
      onDragCancel={() => setActiveTask(null)}
    >
      <div className="flex-1 flex flex-col p-3 w-full max-w-full min-h-0 overflow-hidden">
        {/* Dropped Zone (Appears while dragging) */}
        <DroppedZone isDragging={activeTask !== null} />

        <KanbanBoard className="flex-1">
          {/* Draft Column */}
          <KanbanColumn
            id="draft"
            title="Draft"
            count={draftTasks.length}
            icon={<StatusIcon status="draft" />}
            isReadOnly={isReadOnly}
          >
            {draftTasks.map((t) => (
              <TaskCard
                key={t.id}
                task={t}
                isSelected={selectedTaskId === t.id}
                onClick={() => onSelectTask(t.id)}
                isReadOnly={isReadOnly}
              />
            ))}
          </KanbanColumn>

          {/* Ready Column */}
          <KanbanColumn
            id="ready"
            title="Ready"
            count={readyTasks.length}
            icon={<StatusIcon status="todo" />}
            isReadOnly={isReadOnly}
          >
            {readyTasks.map((t) => (
              <TaskCard
                key={t.id}
                task={t}
                isSelected={selectedTaskId === t.id}
                onClick={() => onSelectTask(t.id)}
                isReadOnly={isReadOnly}
              />
            ))}
          </KanbanColumn>

          {/* Blocked Column */}
          <KanbanColumn
            id="blocked"
            title="Blocked"
            count={blockedTasks.length}
            icon={<StatusIcon status="todo" />}
            isReadOnly={isReadOnly}
          >
            {blockedTasks.map((t) => (
              <TaskCard
                key={t.id}
                task={t}
                isSelected={selectedTaskId === t.id}
                onClick={() => onSelectTask(t.id)}
                isReadOnly={isReadOnly}
              />
            ))}
          </KanbanColumn>

          {/* In progress Column */}
          <KanbanColumn
            id="in_progress"
            title="In progress"
            count={inProgressTasks.length}
            icon={<StatusIcon status="in_progress" />}
            isReadOnly={isReadOnly}
          >
            {inProgressTasks.map((t) => (
              <TaskCard
                key={t.id}
                task={t}
                isSelected={selectedTaskId === t.id}
                onClick={() => onSelectTask(t.id)}
                isReadOnly={isReadOnly}
              />
            ))}
          </KanbanColumn>

          {/* Review Column */}
          <KanbanColumn
            id="review"
            title="Review"
            count={reviewTasks.length}
            icon={<StatusIcon status="review" />}
            isReadOnly={isReadOnly}
          >
            {reviewTasks.map((t) => (
              <TaskCard
                key={t.id}
                task={t}
                isSelected={selectedTaskId === t.id}
                onClick={() => onSelectTask(t.id)}
                isReadOnly={isReadOnly}
              />
            ))}
          </KanbanColumn>

          {/* Done Column (collapsed to narrow by default, expandable on click) */}
          <KanbanColumn
            id="done"
            title="Done"
            count={doneTasks.length}
            icon={<StatusIcon status="done" />}
            isCollapsed={isDoneCollapsed}
            onToggleCollapse={() => setIsDoneCollapsed(!isDoneCollapsed)}
            isReadOnly={isReadOnly}
          >
            {doneTasks.map((t) => (
              <TaskCard
                key={t.id}
                task={t}
                isSelected={selectedTaskId === t.id}
                onClick={() => onSelectTask(t.id)}
                isReadOnly={isReadOnly}
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
              isReadOnly={isReadOnly}
            >
              {droppedTasks.map((t) => (
                <TaskCard
                  key={t.id}
                  task={t}
                  isSelected={selectedTaskId === t.id}
                  onClick={() => onSelectTask(t.id)}
                  isReadOnly={isReadOnly}
                />
              ))}
            </KanbanColumn>
          )}
        </KanbanBoard>
      </div>

      {/* Drag Overlay for floating preview */}
      <DragOverlay>
        {activeTask ? (
          <div className="w-72 shadow-2xl opacity-90 rotate-1 pointer-events-none">
            <TaskCard task={activeTask} />
          </div>
        ) : null}
      </DragOverlay>

      {/* Confirmation Dialog for Verify without evidence from Drag */}
      <ConfirmDialog
        isOpen={pendingVerifyTask !== null}
        title="Verify without evidence"
        description={`Task ${pendingVerifyTask?.id} has no commit evidence. Verifying it will mark it done without recorded commits. Are you sure you want to verify with --force?`}
        confirmLabel="Verify with force"
        confirmVariant="destructive"
        onConfirm={confirmVerifyWithoutEvidence}
        onCancel={() => setPendingVerifyTask(null)}
      />

      {/* Reject Dialog from Drag */}
      {pendingRejectTask && (
        <RejectDialog
          isOpen={true}
          taskId={pendingRejectTask.id}
          onConfirm={confirmRejectWithComment}
          onCancel={() => setPendingRejectTask(null)}
        />
      )}

      {/* Drop Dialog from Drag */}
      {pendingDropTask && (
        <ConfirmDialog
          isOpen={true}
          title={`Drop task ${pendingDropTask.id}`}
          description={`Are you sure you want to drop task ${pendingDropTask.id}?`}
          confirmLabel="Drop task"
          confirmVariant="destructive"
          inputLabel="Reason (optional)"
          inputPlaceholder="e.g. Obsolete"
          onConfirm={confirmDropWithReason}
          onCancel={() => setPendingDropTask(null)}
        />
      )}
    </DndContext>
  );
}
