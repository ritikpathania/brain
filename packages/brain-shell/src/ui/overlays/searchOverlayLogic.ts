/**
 * Pure logic and state management for the /search overlay (Inc 34).
 */
import type { TranscriptSearchMatch } from '../../client/BrainBackendClient.js';
import { overlayListDecision, type OverlayListDecision } from './overlayLogic.js';

export interface SearchItemVM {
  id: string;
  sessionId: string;
  role: string;
  title: string;
  snippet: string;
  score?: number;
}

export const SEARCH_MAX_DISPLAY_ITEMS = 6;

/**
 * Strips the 'session:' prefix from document IDs if present.
 */
export function extractSessionId(raw: string | undefined): string {
  if (!raw) return '';
  if (raw.startsWith('session:')) return raw.slice('session:'.length);
  return raw;
}

/**
 * Maps a canonical TranscriptSearchMatch DTO to the presentation ViewModel.
 */
export function toSearchVM(match: TranscriptSearchMatch): SearchItemVM {
  const rawSessionId = match.sessionId || (match.kind === 'session' ? match.id : '') || match.id || '';
  const sessionId = extractSessionId(rawSessionId);
  const role = match.role || (match.kind === 'session' ? 'session' : 'turn');
  const rawSnippet = match.snippet || match.body || match.title || '';
  const snippet = rawSnippet.replace(/\s+/g, ' ').trim();
  return {
    id: match.id,
    sessionId,
    role,
    title: match.title || '',
    snippet,
    score: match.score,
  };
}

/**
 * Pure reducer turning overlay keyboard actions into query edits.
 */
export function applySearchQueryEdit(query: string, action: string, input: string): string {
  if (action === 'overlay:insert') return query + input;
  if (action === 'overlay:backspace') return query.slice(0, -1);
  return query;
}

/**
 * Clamps selection index safely within match bounds.
 */
export function clampSearchSelection(index: number, count: number): number {
  if (count <= 0) return 0;
  return Math.max(0, Math.min(index, count - 1));
}

/**
 * Pure list navigation decision helper.
 */
export function searchListDecision(
  action: string | null,
  selected: number,
  count: number,
): OverlayListDecision {
  return overlayListDecision(action, selected, count);
}
