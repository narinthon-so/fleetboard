export type ToastType = 'info' | 'error' | 'warning' | 'success';

export interface ToastMessage {
  id: string;
  message: string;
  type: ToastType;
}

type ToastListener = (toasts: ToastMessage[]) => void;

class ToastManager {
  private toasts: ToastMessage[] = [];
  private listeners: Set<ToastListener> = new Set();

  show(message: string, type: ToastType = 'info') {
    const id = `${Date.now()}-${Math.random().toString(36).substring(2, 7)}`;
    this.toasts = [...this.toasts, { id, message, type }];
    this.notify();

    setTimeout(() => {
      this.dismiss(id);
    }, 4000);
  }

  dismiss(id: string) {
    this.toasts = this.toasts.filter((t) => t.id !== id);
    this.notify();
  }

  subscribe(listener: ToastListener) {
    this.listeners.add(listener);
    listener(this.toasts);
    return () => {
      this.listeners.delete(listener);
    };
  }

  private notify() {
    for (const listener of this.listeners) {
      listener(this.toasts);
    }
  }
}

export const toastManager = new ToastManager();

export function toast(message: string, type: ToastType = 'info') {
  toastManager.show(message, type);
}
