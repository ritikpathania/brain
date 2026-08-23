/**
 * Prompt history persistence: ~/.brain/history.jsonl, one JSON entry per line
 * ({mode,value}), newest LAST on disk, loaded reversed so index 0 is newest.
 */
import * as fs from 'fs';
import * as os from 'os';
import * as path from 'path';
import type { PromptInputMode } from '../../contracts/input.js';

export const HISTORY_MAX_ITEMS = 100;

export interface HistoryEntry {
  mode: PromptInputMode;
  value: string;
}

export function historyPath(): string {
  return path.join(os.homedir(), '.brain', 'history.jsonl');
}

export function loadHistory(): HistoryEntry[] {
  try {
    const raw = fs.readFileSync(historyPath(), 'utf8');
    const entries: HistoryEntry[] = [];
    for (const line of raw.split('\n')) {
      if (!line.trim()) continue;
      try {
        const e = JSON.parse(line) as Partial<HistoryEntry>;
        if ((e.mode === 'prompt' || e.mode === 'bash') && typeof e.value === 'string') {
          entries.push({ mode: e.mode, value: e.value });
        }
      } catch {}
    }
    return entries.reverse(); // disk oldest→newest; memory newest-first
  } catch {
    return [];
  }
}

export function appendHistory(entry: HistoryEntry): void {
  try {
    const existing = loadHistory().reverse(); // oldest→newest
    const last = existing[existing.length - 1];
    if (last && last.mode === entry.mode && last.value === entry.value) return;
    const next = [...existing, entry].slice(-HISTORY_MAX_ITEMS);
    const file = historyPath();
    fs.mkdirSync(path.dirname(file), { recursive: true });
    const body = next.map((e) => JSON.stringify(e)).join('\n') + '\n';
    const tmp = `${file}.tmp`;
    fs.writeFileSync(tmp, body, 'utf8');
    fs.renameSync(tmp, file);
  } catch {
    // History is best-effort; never surface I/O errors into the UI loop.
  }
}
