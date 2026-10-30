import React, { useState } from 'react';
import { loginWithToken } from '../lib/api';
import { KeyRound, AlertCircle, ArrowRight } from 'lucide-react';

interface LoginScreenProps {
  onSuccess: () => void;
}

export function LoginScreen({ onSuccess }: LoginScreenProps) {
  const [token, setToken] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [isLoading, setIsLoading] = useState(false);

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault();
    const cleanToken = token.trim();
    if (!cleanToken) {
      setError('Please enter the one-time token');
      return;
    }

    setIsLoading(true);
    setError(null);
    try {
      await loginWithToken(cleanToken);
      onSuccess();
    } catch (err: unknown) {
      if (err instanceof Error) {
        setError(err.message);
      } else {
        setError('Login failed');
      }
    } finally {
      setIsLoading(false);
    }
  }

  return (
    <div className="min-h-screen flex items-center justify-center bg-[var(--background)] p-4">
      <div className="w-full max-w-md p-6 rounded-lg border border-[var(--border)] bg-[var(--card)] shadow-lg flex flex-col gap-5">
        <div className="flex flex-col gap-1.5 text-center items-center">
          <div className="w-10 h-10 rounded-full bg-[var(--muted)] flex items-center justify-center text-[var(--primary)] mb-1">
            <KeyRound className="w-5 h-5" />
          </div>
          <h1 className="text-base font-semibold tracking-tight text-[var(--foreground)] font-mono">
            fleetboard auth
          </h1>
          <p className="text-xs text-[var(--muted-foreground)] max-w-xs">
            Enter the one-time token printed in your terminal by{' '}
            <code className="font-mono text-[var(--foreground)] bg-[var(--muted)] px-1 py-0.5 rounded">
              fb serve
            </code>
          </p>
        </div>

        {error && (
          <div className="flex items-center gap-2 p-2.5 rounded bg-[var(--destructive)]/10 border border-[var(--destructive)]/30 text-[var(--priority-p0)] text-xs">
            <AlertCircle className="w-4 h-4 shrink-0" />
            <span>{error}</span>
          </div>
        )}

        <form onSubmit={handleSubmit} className="flex flex-col gap-3">
          <div className="flex flex-col gap-1">
            <label
              htmlFor="token-input"
              className="text-[11px] font-mono uppercase tracking-wider text-[var(--muted-foreground)]"
            >
              One-Time Token (64-hex)
            </label>
            <input
              id="token-input"
              type="password"
              autoFocus
              autoComplete="off"
              spellCheck={false}
              placeholder="e.g. 9a7b3c2d..."
              value={token}
              onChange={(e) => setToken(e.target.value)}
              className="w-full font-mono text-xs bg-[var(--muted)]/50 border border-[var(--border)] text-[var(--foreground)] rounded px-3 py-2 focus:ring-1 focus:ring-[var(--ring)] focus:outline-none placeholder:text-[var(--muted-foreground)]/60"
            />
          </div>

          <button
            type="submit"
            disabled={isLoading || !token.trim()}
            className="flex items-center justify-center gap-1.5 w-full bg-[var(--primary)] text-[var(--primary-foreground)] hover:opacity-90 disabled:opacity-50 py-2 rounded font-medium text-xs transition-opacity cursor-pointer disabled:cursor-not-allowed"
          >
            <span>{isLoading ? 'Connecting...' : 'Authenticate'}</span>
            <ArrowRight className="w-3.5 h-3.5" />
          </button>
        </form>

        <div className="text-center text-[11px] text-[var(--muted-foreground)] font-mono border-t border-[var(--border)] pt-3">
          loopback only · session valid until logout or restart
        </div>
      </div>
    </div>
  );
}
