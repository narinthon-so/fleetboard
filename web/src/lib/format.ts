import type { Task } from '../types/api';

export function formatDuration(dateStr: string): string {
  const diffMs = Math.max(0, Date.now() - new Date(dateStr).getTime());
  const totalMins = Math.floor(diffMs / 60000);
  if (totalMins < 60) {
    return `${totalMins}m`;
  }
  const hours = Math.floor(totalMins / 60);
  const mins = totalMins % 60;
  return mins > 0 ? `${hours}h ${mins}m` : `${hours}h`;
}

export function getProviderMonogram(provider: string | null, kind: 'agent' | 'human'): string {
  if (kind === 'human') return 'H';
  if (!provider) return '?';
  const p = provider.toLowerCase();
  if (p.includes('claude')) return 'C';
  if (p.includes('codex')) return 'X';
  if (p.includes('antigravity') || p.includes('agy')) return 'A';
  return '?';
}

export function formatShortId(externalId: string | null, label: string): string {
  if (externalId && externalId.trim().length > 0) {
    return externalId.trim().slice(0, 7);
  }
  const parts = label.split(/\s+/);
  if (parts.length > 1) {
    return parts[parts.length - 1].slice(0, 7);
  }
  return label.slice(0, 7);
}

export function getNeedsMeReason(task: Task): string {
  const runtimeStatus = task.runtime_status || task.runtime?.status;
  const perms = task.pending_permissions ?? task.runtime?.pending_permissions ?? 0;

  // Pending permissions from runtime
  if (perms > 0 || runtimeStatus === 'permission' || runtimeStatus === 'waiting-for-permission') {
    const count = perms > 0 ? perms : 1;
    return count === 1 ? '1 permission pending' : `${count} permissions pending`;
  }

  if (task.status === 'review') {
    if (task.derived.no_evidence) {
      return 'review · no commits';
    }
    return 'review pending';
  }
  if (task.derived.plan_pending) {
    return 'plan pending';
  }
  if (task.derived.stale) {
    if (runtimeStatus === 'gone') {
      return 'stale · agent gone';
    }
    const age = task.claim ? formatDuration(task.claim.last_activity || task.claim.claimed_at) : '';
    return age ? `stale ${age}` : 'stale claim';
  }
  if (task.status === 'draft') {
    return 'draft';
  }
  if (task.derived.orphaned) {
    return 'orphaned';
  }
  return 'needs attention';
}
