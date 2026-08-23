/**
 * Pure composer reducer: editing buffer, cursor ops, undo stack, large-paste
 * truncation, and per-mode history navigation. No I/O, no React — fully
 * unit-testable; PromptInput binds translateKey → reduceComposer.
 */
import type { PromptInputMode } from '../../contracts/input.js';
import { processPaste, expandPastedPlaceholders } from './paste.js';
import type { HistoryEntry } from './historyStore.js';

export type { HistoryEntry } from './historyStore.js';

export interface ComposerSnapshot {
  value: string;
  cursor: number;
}

export interface ComposerState extends ComposerSnapshot {
  pastedContents: Record<string, string>;
  pasteCounter: number;
  undoStack: ComposerSnapshot[];
  history: HistoryEntry[]; // newest-first
  historyIndex: number; // -1 while composing
  historyDraft: string;
  /** Mode captured when browsing began; keeps '!'-started browsing in bash entries. */
  historyBrowseMode?: PromptInputMode;
}

export type ComposerAction =
  | { type: 'insert'; text: string }
  | { type: 'newline' }
  | { type: 'backspace' }
  | { type: 'delete' }
  | { type: 'left' }
  | { type: 'right' }
  | { type: 'home' }
  | { type: 'end' }
  | { type: 'kill_to_end' }
  | { type: 'kill_to_start' }
  | { type: 'delete_word_back' }
  | { type: 'undo' }
  | { type: 'history_up' }
  | { type: 'history_down' }
  | { type: 'submit_done'; entry: HistoryEntry };

const UNDO_LIMIT = 50;

export function createComposerState(history: HistoryEntry[] = []): ComposerState {
  return {
    value: '',
    cursor: 0,
    pastedContents: {},
    pasteCounter: 0,
    undoStack: [],
    history,
    historyIndex: -1,
    historyDraft: '',
  };
}

export function modeOf(value: string): PromptInputMode {
  return value.startsWith('!') ? 'bash' : 'prompt';
}

export function wordBackStart(value: string, cursor: number): number {
  let i = cursor;
  while (i > 0 && /\s/.test(value[i - 1]!)) i--;
  while (i > 0 && !/\s/.test(value[i - 1]!)) i--;
  return i;
}

function pushUndo(state: ComposerState, prev: ComposerState): ComposerState {
  return {
    ...state,
    undoStack: [
      ...state.undoStack.slice(-(UNDO_LIMIT - 1)),
      { value: prev.value, cursor: prev.cursor },
    ],
  };
}

/** Submit-time view of the buffer: placeholders replaced with their full text. */
export function expandedValue(state: ComposerState): string {
  return expandPastedPlaceholders(state.value, state.pastedContents);
}

function insertRaw(state: ComposerState, text: string): ComposerState {
  const result = processPaste(text, state.pasteCounter);
  const pastedContents =
    result.stored !== undefined
      ? { ...state.pastedContents, [result.stored.id]: result.stored.content }
      : state.pastedContents;
  return {
    ...state,
    value: state.value.slice(0, state.cursor) + result.inserted + state.value.slice(state.cursor),
    cursor: state.cursor + result.inserted.length,
    pastedContents,
    pasteCounter: result.nextCounter,
  };
}

function candidatesFor(state: ComposerState, mode: PromptInputMode): number[] {
  const idx: number[] = [];
  state.history.forEach((entry, i) => {
    if (entry.mode === mode) idx.push(i);
  });
  return idx;
}

export function reduceComposer(state: ComposerState, action: ComposerAction): ComposerState {
  switch (action.type) {
    case 'insert': {
      if (action.text.length === 0) return state;
      return pushUndo(insertRaw(state, action.text), state);
    }
    case 'newline':
      return pushUndo(insertRaw(state, '\n'), state);
    case 'backspace': {
      if (state.cursor === 0) return state;
      return pushUndo(
        {
          ...state,
          value: state.value.slice(0, state.cursor - 1) + state.value.slice(state.cursor),
          cursor: state.cursor - 1,
        },
        state,
      );
    }
    case 'delete': {
      // Forward delete (fn+backspace / Del): remove the char at the cursor.
      if (state.cursor >= state.value.length) return state;
      return pushUndo(
        {
          ...state,
          value: state.value.slice(0, state.cursor) + state.value.slice(state.cursor + 1),
        },
        state,
      );
    }
    case 'left':
      return { ...state, cursor: Math.max(0, state.cursor - 1) };
    case 'right':
      return { ...state, cursor: Math.min(state.value.length, state.cursor + 1) };
    case 'home':
      return { ...state, cursor: 0 };
    case 'end':
      return { ...state, cursor: state.value.length };
    case 'kill_to_end':
      if (state.cursor >= state.value.length) return state;
      return pushUndo({ ...state, value: state.value.slice(0, state.cursor) }, state);
    case 'kill_to_start': {
      if (state.cursor === 0) return state;
      return pushUndo({ ...state, value: state.value.slice(state.cursor), cursor: 0 }, state);
    }
    case 'delete_word_back': {
      const start = wordBackStart(state.value, state.cursor);
      if (start === state.cursor) return state;
      return pushUndo(
        {
          ...state,
          value: state.value.slice(0, start) + state.value.slice(state.cursor),
          cursor: start,
        },
        state,
      );
    }
    case 'undo': {
      const prev = state.undoStack[state.undoStack.length - 1];
      if (!prev) return state;
      return { ...state, value: prev.value, cursor: prev.cursor, undoStack: state.undoStack.slice(0, -1) };
    }
    case 'history_up': {
      // Browse mode is captured once, at the moment browsing starts: a user
      // who typed '!' and pressed Up browses bash entries even though the
      // restored bare values no longer start with '!'.
      const browseMode =
        state.historyIndex === -1
          ? modeOf(state.value)
          : state.historyBrowseMode ?? modeOf(state.value);
      const cands = candidatesFor(state, browseMode);
      if (cands.length === 0) return state;
      const pos = state.historyIndex === -1 ? -1 : cands.indexOf(state.historyIndex);
      const newPos = pos < 0 ? 0 : Math.min(pos + 1, cands.length - 1);
      const chosenIdx = cands[newPos]!;
      const chosen = state.history[chosenIdx]!;
      return {
        ...state,
        value: chosen.value,
        cursor: chosen.value.length,
        historyBrowseMode: browseMode,
        historyDraft: state.historyIndex === -1 ? state.value : state.historyDraft,
        historyIndex: chosenIdx,
      };
    }
    case 'history_down': {
      if (state.historyIndex === -1) return state;
      const cands = candidatesFor(state, state.historyBrowseMode ?? modeOf(state.value));
      const pos = cands.indexOf(state.historyIndex);
      if (pos <= 0) {
        return {
          ...state,
          value: state.historyDraft,
          cursor: state.historyDraft.length,
          historyIndex: -1,
          historyBrowseMode: undefined,
        };
      }
      const nextIdx = cands[pos - 1]!;
      const chosen = state.history[nextIdx]!;
      return { ...state, value: chosen.value, cursor: chosen.value.length, historyIndex: nextIdx };
    }
    case 'submit_done':
      return {
        ...state,
        value: '',
        cursor: 0,
        pasteCounter: 0,
        pastedContents: {},
        undoStack: [],
        historyIndex: -1,
        historyDraft: '',
        historyBrowseMode: undefined,
        history: [
          action.entry,
          ...state.history.filter(
            (e) => !(e.mode === action.entry.mode && e.value === action.entry.value),
          ),
        ],
      };
    default:
      return state;
  }
}
