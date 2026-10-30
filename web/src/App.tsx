import { useState, useEffect, useCallback, useMemo } from 'react';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { getBoard, ApiError, verifyTask, rejectTask } from './lib/api';
import { TopBar, type BoardFilters, type SSEStatus } from './components/TopBar';
import { NeedsMeStrip } from './components/NeedsMeStrip';
import { BoardView } from './components/BoardView';
import { CardDetail } from './components/CardDetail';
import { LoginScreen } from './components/LoginScreen';
import { ShortcutsDialog } from './components/ShortcutsDialog';
import { ToastContainer } from './components/Toast';
import { toast } from './lib/toast';
import { ConfirmDialog, RejectDialog } from './components/ActionDialogs';
import { AlertTriangle, WifiOff } from 'lucide-react';
import { cn } from './lib/utils';

export function App() {
  return (
    <>
      <BoardApp />
      <ToastContainer />
    </>
  );
}

function BoardApp() {
  const queryClient = useQueryClient();

  // Theme state: dark default
  const [isDark, setIsDark] = useState<boolean>(() => {
    return !document.documentElement.classList.contains('light');
  });

  const toggleTheme = useCallback(() => {
    setIsDark((prev) => {
      const next = !prev;
      if (next) {
        document.documentElement.classList.remove('light');
        document.documentElement.classList.add('dark');
      } else {
        document.documentElement.classList.remove('dark');
        document.documentElement.classList.add('light');
      }
      return next;
    });
  }, []);

  // Filter state synced with URL search params
  const [filters, setFilters] = useState<BoardFilters>(() => {
    const params = new URLSearchParams(window.location.search);
    return {
      project: params.get('project') || '',
      instance: params.get('instance') || '',
      agent: params.get('agent') || '',
      host: params.get('host') || '',
      search: params.get('search') || '',
      showDropped: params.get('show_dropped') === 'true',
    };
  });

  // Selected task ID for Detail Sheet, synced with URL pathname (/task/fb-12)
  const [selectedTaskId, setSelectedTaskId] = useState<string | null>(() => {
    const match = window.location.pathname.match(/\/task\/([a-zA-Z0-9_-]+)/);
    return match ? match[1] : null;
  });

  const [focusedTaskId, setFocusedTaskId] = useState<string | null>(null);

  // Keyboard shortcuts dialog state
  const [isShortcutsOpen, setIsShortcutsOpen] = useState(false);

  // Keyboard shortcut confirmation dialogs for 'v' and 'r'
  const [keyVerifyTask, setKeyVerifyTask] = useState<{ id: string; force: boolean; description: string } | null>(null);
  const [keyRejectTask, setKeyRejectTask] = useState<string | null>(null);

  // SSE status
  const [sseStatus, setSseStatus] = useState<SSEStatus>('connected');
  const [isLongDisconnected, setIsLongDisconnected] = useState(false);
  const [isDaemonDown, setIsDaemonDown] = useState(false);

  // Auth needed state
  const [isAuthNeeded, setIsAuthNeeded] = useState(false);

  const isReadOnly = isDaemonDown || sseStatus === 'reconnecting';

  // Sync state to URL
  useEffect(() => {
    const params = new URLSearchParams();
    if (filters.project) params.set('project', filters.project);
    if (filters.instance) params.set('instance', filters.instance);
    if (filters.agent) params.set('agent', filters.agent);
    if (filters.host) params.set('host', filters.host);
    if (filters.search) params.set('search', filters.search);
    if (filters.showDropped) params.set('show_dropped', 'true');

    const queryString = params.toString() ? `?${params.toString()}` : '';
    const path = selectedTaskId ? `/task/${selectedTaskId}` : '/';
    const newUrl = `${path}${queryString}`;

    if (window.location.pathname + window.location.search !== newUrl) {
      window.history.replaceState(null, '', newUrl);
    }
  }, [filters, selectedTaskId]);

  // Handle browser back/forward buttons
  useEffect(() => {
    function handlePopState() {
      const params = new URLSearchParams(window.location.search);
      setFilters({
        project: params.get('project') || '',
        instance: params.get('instance') || '',
        agent: params.get('agent') || '',
        host: params.get('host') || '',
        search: params.get('search') || '',
        showDropped: params.get('show_dropped') === 'true',
      });
      const match = window.location.pathname.match(/\/task\/([a-zA-Z0-9_-]+)/);
      setSelectedTaskId(match ? match[1] : null);
    }
    window.addEventListener('popstate', handlePopState);
    return () => window.removeEventListener('popstate', handlePopState);
  }, []);

  // Board Data query
  const {
    data: boardData,
    isLoading,
    refetch,
  } = useQuery({
    queryKey: ['board'],
    queryFn: async () => {
      try {
        const data = await getBoard();
        setIsAuthNeeded(false);
        setIsDaemonDown(false);
        return data;
      } catch (err) {
        if (err instanceof ApiError && err.status === 401) {
          setIsAuthNeeded(true);
        } else {
          setIsDaemonDown(true);
        }
        throw err;
      }
    },
    retry: false,
  });

  // Setup SSE connection
  useEffect(() => {
    if (isAuthNeeded) return;

    let eventSource: EventSource | null = null;
    let reconnectTimeout: ReturnType<typeof setTimeout> | null = null;
    let longDisconnectTimer: ReturnType<typeof setTimeout> | null = null;

    function connect() {
      eventSource = new EventSource('/api/events', { withCredentials: true });

      eventSource.onopen = () => {
        setSseStatus('connected');
        setIsLongDisconnected(false);
        setIsDaemonDown(false);
        if (longDisconnectTimer) clearTimeout(longDisconnectTimer);
      };

      eventSource.onmessage = (e) => {
        try {
          const payload = JSON.parse(e.data);
          if (payload.type === 'changed') {
            queryClient.invalidateQueries({ queryKey: ['board'] });
            queryClient.invalidateQueries({ queryKey: ['task'] });
          }
        } catch {
          // Keepalive or non-json message
        }
      };

      eventSource.onerror = () => {
        eventSource?.close();
        setSseStatus('reconnecting');

        if (!longDisconnectTimer) {
          longDisconnectTimer = setTimeout(() => {
            setIsLongDisconnected(true);
          }, 5000);
        }

        reconnectTimeout = setTimeout(() => {
          connect();
        }, 3000);
      };
    }

    connect();

    return () => {
      if (eventSource) eventSource.close();
      if (reconnectTimeout) clearTimeout(reconnectTimeout);
      if (longDisconnectTimer) clearTimeout(longDisconnectTimer);
    };
  }, [isAuthNeeded, queryClient]);

  // List of all visible tasks for j/k keyboard navigation
  const taskIdsList = useMemo(() => {
    if (!boardData) return [];
    return boardData.tasks
      .filter((t) => filters.showDropped || t.status !== 'dropped')
      .map((t) => t.id);
  }, [boardData, filters.showDropped]);

  // Global Keyboard shortcuts
  useEffect(() => {
    let gPressed = false;
    let gTimeout: ReturnType<typeof setTimeout> | null = null;

    function handleKeyDown(e: KeyboardEvent) {
      if (
        document.activeElement?.tagName === 'INPUT' ||
        document.activeElement?.tagName === 'TEXTAREA' ||
        document.activeElement?.tagName === 'SELECT'
      ) {
        return;
      }

      if (e.key === '?') {
        e.preventDefault();
        setIsShortcutsOpen((prev) => !prev);
        return;
      }

      if (e.key === 'Escape') {
        if (isShortcutsOpen) {
          setIsShortcutsOpen(false);
        } else if (keyVerifyTask) {
          setKeyVerifyTask(null);
        } else if (keyRejectTask) {
          setKeyRejectTask(null);
        } else if (selectedTaskId) {
          setSelectedTaskId(null);
        }
        return;
      }

      if (e.key === 'g') {
        gPressed = true;
        if (gTimeout) clearTimeout(gTimeout);
        gTimeout = setTimeout(() => {
          gPressed = false;
        }, 1000);
        return;
      }

      if (e.key === 'n' && gPressed) {
        gPressed = false;
        // Jump to first Needs me card
        const needsMeTask = boardData?.tasks.find((t) => t.derived.needs_me);
        if (needsMeTask) {
          setSelectedTaskId(needsMeTask.id);
        }
        return;
      }

      if (e.key === 'j') {
        // Next card
        e.preventDefault();
        if (taskIdsList.length === 0) return;
        if (!focusedTaskId) {
          setFocusedTaskId(taskIdsList[0]);
        } else {
          const idx = taskIdsList.indexOf(focusedTaskId);
          if (idx >= 0 && idx < taskIdsList.length - 1) {
            setFocusedTaskId(taskIdsList[idx + 1]);
          }
        }
        return;
      }

      if (e.key === 'k') {
        // Prev card
        e.preventDefault();
        if (taskIdsList.length === 0) return;
        if (!focusedTaskId) {
          setFocusedTaskId(taskIdsList[taskIdsList.length - 1]);
        } else {
          const idx = taskIdsList.indexOf(focusedTaskId);
          if (idx > 0) {
            setFocusedTaskId(taskIdsList[idx - 1]);
          }
        }
        return;
      }

      if (e.key === 'Enter') {
        if (!selectedTaskId && focusedTaskId && taskIdsList.includes(focusedTaskId)) {
          e.preventDefault();
          setSelectedTaskId(focusedTaskId);
        }
        return;
      }

      // 'v' verifies the open card (confirm first)
      if (e.key === 'v') {
        if (!selectedTaskId || isReadOnly) return;
        const task = boardData?.tasks.find((t) => t.id === selectedTaskId);
        if (task && task.status === 'review') {
          e.preventDefault();
          const hasEvidence = task.evidence && task.evidence.commits > 0;
          if (task.kind === 'code' && (!hasEvidence || task.derived.no_evidence)) {
            setKeyVerifyTask({
              id: task.id,
              force: true,
              description: `Task ${task.id} has no commit evidence. Verifying without evidence requires force. Verify with --force?`,
            });
          } else {
            setKeyVerifyTask({
              id: task.id,
              force: false,
              description: task.kind === 'note'
                ? `Verify note ${task.id} and mark it done?`
                : `Verify task ${task.id} (${task.evidence?.commits} commits) and mark it done?`,
            });
          }
        }
        return;
      }

      // 'r' rejects the open card (confirm first with comment)
      if (e.key === 'r') {
        if (!selectedTaskId || isReadOnly) return;
        const task = boardData?.tasks.find((t) => t.id === selectedTaskId);
        if (task && task.status === 'review') {
          e.preventDefault();
          setKeyRejectTask(task.id);
        }
        return;
      }
    }

    window.addEventListener('keydown', handleKeyDown);
    return () => {
      window.removeEventListener('keydown', handleKeyDown);
      if (gTimeout) clearTimeout(gTimeout);
    };
  }, [boardData, selectedTaskId, focusedTaskId, taskIdsList, isShortcutsOpen, keyVerifyTask, keyRejectTask, isReadOnly]);

  async function handleKeyVerifyConfirm() {
    if (!keyVerifyTask) return;
    const { id, force } = keyVerifyTask;
    setKeyVerifyTask(null);
    try {
      await verifyTask(id, force);
      toast(`${id} verified and marked done`, 'success');
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: ['board'] }),
        queryClient.invalidateQueries({ queryKey: ['task', id] }),
      ]);
    } catch (err) {
      toast(err instanceof Error ? err.message : 'Verify failed', 'error');
    }
  }

  async function handleKeyRejectConfirm(comment: string) {
    if (!keyRejectTask) return;
    const id = keyRejectTask;
    setKeyRejectTask(null);
    try {
      await rejectTask(id, comment);
      toast(`${id} rejected back to in_progress`, 'info');
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: ['board'] }),
        queryClient.invalidateQueries({ queryKey: ['task', id] }),
      ]);
    } catch (err) {
      toast(err instanceof Error ? err.message : 'Reject failed', 'error');
    }
  }

  // If authentication is needed, show only LoginScreen
  if (isAuthNeeded) {
    return (
      <LoginScreen
        onSuccess={() => {
          setIsAuthNeeded(false);
          refetch();
        }}
      />
    );
  }

  return (
    <div className="min-h-screen flex flex-col bg-[var(--background)] text-[var(--foreground)] overflow-hidden font-sans">
      {/* Daemon Down Banner */}
      {isDaemonDown && (
        <div
          role="alert"
          className="w-full bg-[var(--priority-p0)] text-[var(--background)] font-medium text-xs px-4 py-2 flex items-center justify-between shadow-md shrink-0 select-none"
        >
          <div className="flex items-center gap-2">
            <WifiOff className="w-4 h-4 shrink-0" />
            <span>fb serve is not running — start it with <code className="bg-black/20 px-1 py-0.5 rounded font-mono">fb serve</code></span>
          </div>
          <button
            type="button"
            onClick={() => refetch()}
            className="text-[11px] font-semibold underline hover:opacity-80 cursor-pointer"
          >
            Retry
          </button>
        </div>
      )}

      {/* SSE Reconnecting Banner (>5s disconnect) */}
      {!isDaemonDown && isLongDisconnected && (
        <div
          role="status"
          className="w-full bg-[var(--status-review)] text-[var(--background)] font-medium text-xs px-4 py-1.5 flex items-center gap-2 shrink-0 select-none"
        >
          <AlertTriangle className="w-3.5 h-3.5 shrink-0" />
          <span>Real-time connection lost. Reconnecting... (showing cached board)</span>
        </div>
      )}

      {/* Main Board UI */}
      <div className={cn('flex flex-col flex-1 min-h-0', isDaemonDown && 'opacity-60 pointer-events-none')}>
        {/* TopBar with filters */}
        <TopBar
          boardData={boardData || null}
          filters={filters}
          onFilterChange={setFilters}
          sseStatus={isDaemonDown ? 'daemon_down' : sseStatus}
          isDark={isDark}
          onToggleTheme={toggleTheme}
        />

        {/* Needs-me strip: always visible, ignoring column filters */}
        <NeedsMeStrip
          tasks={boardData?.tasks || []}
          onSelectTask={(id) => { setFocusedTaskId(id); setSelectedTaskId(id); }}
        />

        {/* Kanban Board Columns View */}
        <main className="flex-1 overflow-hidden flex flex-col min-h-0">
          <BoardView
            boardData={boardData || null}
            filters={filters}
            isLoading={isLoading && !boardData}
            selectedTaskId={selectedTaskId || focusedTaskId}
            onSelectTask={(id) => { setFocusedTaskId(id); setSelectedTaskId(id); }}
            isReadOnly={isReadOnly}
          />
        </main>
      </div>

      {/* Card Detail Sheet */}
      {selectedTaskId && (
        <CardDetail
          taskId={selectedTaskId}
          onClose={() => setSelectedTaskId(null)}
          onNavigateTask={(id) => setSelectedTaskId(id)}
          allTasks={boardData?.tasks || []}
          isReadOnly={isReadOnly}
        />
      )}

      {/* Keyboard Shortcuts Dialog */}
      <ShortcutsDialog
        isOpen={isShortcutsOpen}
        onClose={() => setIsShortcutsOpen(false)}
      />

      {/* Keyboard shortcut 'v' confirm dialog */}
      {keyVerifyTask && (
        <ConfirmDialog
          isOpen={true}
          title={keyVerifyTask.force ? 'Verify without evidence' : `Verify task ${keyVerifyTask.id}`}
          description={keyVerifyTask.description}
          confirmLabel={keyVerifyTask.force ? 'Verify with force' : 'Verify and Complete'}
          confirmVariant={keyVerifyTask.force ? 'destructive' : 'primary'}
          onConfirm={handleKeyVerifyConfirm}
          onCancel={() => setKeyVerifyTask(null)}
        />
      )}

      {/* Keyboard shortcut 'r' reject dialog */}
      {keyRejectTask && (
        <RejectDialog
          isOpen={true}
          taskId={keyRejectTask}
          onConfirm={handleKeyRejectConfirm}
          onCancel={() => setKeyRejectTask(null)}
        />
      )}
    </div>
  );
}
