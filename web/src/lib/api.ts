import type { ApiResponse, BoardData, TaskDetail } from '../types/api';

export class ApiError extends Error {
  code: string;
  status: number;

  constructor(code: string, message: string, status: number) {
    super(message);
    this.name = 'ApiError';
    this.code = code;
    this.status = status;
  }
}

async function request<T>(path: string, options: RequestInit = {}): Promise<T> {
  const headers = new Headers(options.headers);
  headers.set('Accept', 'application/json');

  const res = await fetch(path, {
    ...options,
    headers,
    credentials: 'include',
  });

  if (res.status === 401) {
    throw new ApiError('UNAUTHORIZED', 'Authentication required', 401);
  }

  let json: ApiResponse<T>;
  try {
    json = await res.json();
  } catch {
    throw new ApiError('INTERNAL', `Server responded with status ${res.status}`, res.status);
  }

  if (!res.ok || !json.ok || !json.data) {
    throw new ApiError(
      json.error?.code || 'ERROR',
      json.error?.message || `Request failed with status ${res.status}`,
      res.status
    );
  }

  return json.data;
}

export async function getBoard(): Promise<BoardData> {
  return request<BoardData>('/api/board');
}

export async function getTaskDetail(id: string): Promise<TaskDetail> {
  const data = await request<{ task: TaskDetail }>(`/api/tasks/${id}`);
  return data.task;
}

export async function loginWithToken(token: string): Promise<{ message: string }> {
  const headers = new Headers({
    'Content-Type': 'application/json',
    'X-Requested-With': 'fleetboard',
  });

  const res = await fetch('/api/login', {
    method: 'POST',
    headers,
    credentials: 'include',
    body: JSON.stringify({ token: token.trim() }),
  });

  const json: ApiResponse<{ message: string }> = await res.json();
  if (!res.ok || !json.ok) {
    throw new ApiError(
      json.error?.code || 'UNAUTHORIZED',
      json.error?.message || 'Invalid or expired token',
      res.status
    );
  }

  return json.data!;
}

export async function logout(): Promise<void> {
  const headers = new Headers({
    'X-Requested-With': 'fleetboard',
  });

  await fetch('/api/logout', {
    method: 'POST',
    headers,
    credentials: 'include',
  });
}
