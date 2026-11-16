/**
 * Presentation-only values. These are already-projected frontend state, never
 * UDS frames, storage records, or local tool-execution instructions.
 */
export type TerminalEntry = ConversationEntry;

export type TerminalConnectionState = 'checking' | 'ready' | 'disconnected' | 'reconnecting' | 'incompatible' | 'failed';

export type BrainTerminalModel = Readonly<{
  entries: readonly TerminalEntry[];
  draft: string;
  draftCursor: number;
  connection: TerminalConnectionState;
  sessionTitle?: string;
  generating: boolean;
  cancelling: boolean;
  focus: FrontendInteractionSnapshot['focus'];
  permission?: Readonly<{ label: string; summary: string }>;
  contextSummary?: Readonly<{ retrievalCount: number; contextTokenCount: number }>;
  sessionPickerOpen: boolean;
  selectedSessionIndex: number;
  sessions: readonly Readonly<{ sessionId: string; title: string; messageCount: number }> [];
  followTranscript: boolean;
}>;

export function emptyTerminalModel(): BrainTerminalModel {
  return Object.freeze({
    entries: Object.freeze([]),
    draft: '',
    draftCursor: 0,
    connection: 'checking',
    generating: false,
    cancelling: false,
    focus: 'composer',
    sessionPickerOpen: false,
    selectedSessionIndex: 0,
    sessions: Object.freeze([]),
    followTranscript: true,
  });
}

/** Converts frontend-neutral state into the only data terminal components read. */
export function toTerminalModel(state: FrontendInteractionSnapshot): BrainTerminalModel {
  const end = state.followTranscript
    ? state.transcript.length
    : Math.max(state.transcript.length === 0 ? 0 : 1, state.transcript.length - state.scrollOffset);
  const start = Math.max(0, end - 24);
  return Object.freeze({
    entries: Object.freeze(state.transcript.slice(start, end)),
    draft: state.composer.draft,
    draftCursor: state.composer.cursor,
    connection: state.connection,
    ...(state.session === undefined ? {} : { sessionTitle: state.session.title }),
    generating: state.generating,
    cancelling: state.cancelling,
    focus: state.focus,
    sessionPickerOpen: state.sessionPickerOpen,
    selectedSessionIndex: state.selectedSessionIndex,
    sessions: Object.freeze(state.sessions.map(session => Object.freeze({
      sessionId: session.sessionId,
      title: session.title,
      messageCount: session.messageCount,
    }))),
    followTranscript: state.followTranscript,
    ...(state.permission === undefined ? {} : { permission: Object.freeze({ label: state.permission.label, summary: state.permission.summary }) }),
    ...(state.contextSummary === undefined ? {} : { contextSummary: state.contextSummary }),
  });
}
import type { ConversationEntry, FrontendInteractionSnapshot } from '../state/interactions.js';
