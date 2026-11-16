export type ComposerState = Readonly<{ draft: string; cursor: number }>;
export type CursorDirection = 'left' | 'right' | 'up' | 'down' | 'home' | 'end';

export function emptyComposerState(): ComposerState {
  return Object.freeze({ draft: '', cursor: 0 });
}

export function replaceDraft(_state: ComposerState, draft: string): ComposerState {
  return Object.freeze({ draft, cursor: draft.length });
}

export function appendToDraft(state: ComposerState, input: string): ComposerState {
  return Object.freeze({ draft: `${state.draft.slice(0, state.cursor)}${input}${state.draft.slice(state.cursor)}`, cursor: state.cursor + input.length });
}

export function appendNewline(state: ComposerState): ComposerState {
  return appendToDraft(state, '\n');
}

export function deletePreviousCharacter(state: ComposerState): ComposerState {
  const previous = Array.from(state.draft.slice(0, state.cursor)).at(-1)?.length ?? 0;
  const cursor = state.cursor - previous;
  return Object.freeze({ draft: state.draft.slice(0, cursor) + state.draft.slice(state.cursor), cursor });
}

export function moveDraftCursor(state: ComposerState, direction: CursorDirection): ComposerState {
  const lineStart = state.draft.slice(0, state.cursor).lastIndexOf('\n') + 1;
  const nextBreak = state.draft.indexOf('\n', state.cursor);
  const lineEnd = nextBreak < 0 ? state.draft.length : nextBreak;
  let cursor = state.cursor;
  if (direction === 'left') cursor -= Array.from(state.draft.slice(0, cursor)).at(-1)?.length ?? 0;
  if (direction === 'right') cursor += Array.from(state.draft.slice(cursor))[0]?.length ?? 0;
  if (direction === 'home') cursor = lineStart;
  if (direction === 'end') cursor = lineEnd;
  if (direction === 'up' && lineStart > 0) {
    const previousEnd = lineStart - 1;
    const previousStart = state.draft.slice(0, previousEnd).lastIndexOf('\n') + 1;
    cursor = Math.min(previousEnd, previousStart + state.cursor - lineStart);
  }
  if (direction === 'down' && lineEnd < state.draft.length) {
    const nextStart = lineEnd + 1;
    const nextEnd = state.draft.indexOf('\n', nextStart);
    cursor = Math.min(nextEnd < 0 ? state.draft.length : nextEnd, nextStart + state.cursor - lineStart);
  }
  // Keep an offset from splitting a surrogate pair after vertical navigation.
  if (cursor > 0 && cursor < state.draft.length && /[\uDC00-\uDFFF]/.test(state.draft[cursor]!)) cursor -= 1;
  return Object.freeze({ ...state, cursor });
}
