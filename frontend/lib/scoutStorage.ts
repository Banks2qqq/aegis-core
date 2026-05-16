import type { ScoutResult } from './api';

const KEY = 'aegis_last_scout';

export type StoredScoutRun = Omit<ScoutResult, 'completed_at'> & { completed_at: string };

export function saveLastScoutRun(result: ScoutResult): StoredScoutRun {
  const stored: StoredScoutRun = {
    ...result,
    completed_at: result.completed_at
      ? new Date(result.completed_at * 1000).toISOString()
      : new Date().toISOString(),
  };
  try {
    localStorage.setItem(KEY, JSON.stringify(stored));
  } catch {
    /* ignore */
  }
  if (typeof window !== 'undefined') {
    window.dispatchEvent(new CustomEvent('aegis_scout_complete', { detail: stored }));
  }
  return stored;
}

export function loadLastScoutRun(): StoredScoutRun | null {
  try {
    const raw = localStorage.getItem(KEY);
    if (!raw) return null;
    return JSON.parse(raw) as StoredScoutRun;
  } catch {
    return null;
  }
}
