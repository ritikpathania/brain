import { describe, it, expect } from 'bun:test';
import {
  createComposerState, reduceComposer, modeOf, wordBackStart, expandedValue,
} from '../../ui/composer/composerState.js';
import { processPaste, expandPastedPlaceholders } from '../../ui/composer/paste.js';
import { translateKey } from '../../ui/composer/translateKey.js';

const HIST = [
  { mode: 'prompt' as const, value: 'second prompt' },
  { mode: 'prompt' as const, value: 'first prompt' },
  { mode: 'bash' as const, value: 'ls -la' },
];

describe('composer modes', () => {
  it('derives mode from leading bang like the reference inputModes contract', () => {
    expect(modeOf('')).toBe('prompt');
    expect(modeOf('hi')).toBe('prompt');
    expect(modeOf('!ls')).toBe('bash');
    expect(modeOf('! ls')).toBe('bash');
    expect(modeOf("don't!")).toBe('prompt'); // only position 0 counts
  });
});

describe('editing actions', () => {
  it('inserts at cursor and moves it', () => {
    let s = createComposerState();
    s = reduceComposer(s, { type: 'insert', text: 'ab' });
    s = reduceComposer(s, { type: 'left' });
    s = reduceComposer(s, { type: 'insert', text: 'X' });
    expect(s.value).toBe('aXb');
    expect(s.cursor).toBe(2);
  });

  it('backspace/delete respect boundaries without pushing no-op undo entries', () => {
    let s = createComposerState();
    s = reduceComposer(s, { type: 'insert', text: 'ab' });
    const depth = s.undoStack.length;
    s = reduceComposer(s, { type: 'left' });
    s = reduceComposer(s, { type: 'delete' }); // removes 'b'
    expect(s.value).toBe('a');
    s = reduceComposer(s, { type: 'backspace' }); // removes 'a'
    expect(s.value).toBe('');
    s = reduceComposer(s, { type: 'backspace' }); // boundary no-op
    expect(s.value).toBe('');
    expect(s.undoStack.length).toBeLessThanOrEqual(depth + 2);
  });

  it('kills to start/end and deletes word-back', () => {
    let s = createComposerState();
    s = reduceComposer(s, { type: 'insert', text: 'one two three' });
    s = reduceComposer(s, { type: 'kill_to_start' });
    expect(s.value).toBe('');
    expect(s.cursor).toBe(0);

    s = createComposerState();
    s = reduceComposer(s, { type: 'insert', text: 'one two three' });
    s = reduceComposer(s, { type: 'left' });
    s = reduceComposer(s, { type: 'left' });
    s = reduceComposer(s, { type: 'left' });
    s = reduceComposer(s, { type: 'left' });
    s = reduceComposer(s, { type: 'left' }); // cursor before 'three'
    s = reduceComposer(s, { type: 'delete_word_back' }); // kills 'two '
    expect(s.value).toBe('one three');

    s = createComposerState();
    s = reduceComposer(s, { type: 'insert', text: 'keep me' });
    s = reduceComposer(s, { type: 'home' });
    s = reduceComposer(s, { type: 'kill_to_end' });
    expect(s.value).toBe('');
  });

  it('wordBackStart skips trailing spaces then the word run', () => {
    expect(wordBackStart('foo bar   ', 10)).toBe(4);
    expect(wordBackStart('foo bar', 7)).toBe(4);
    expect(wordBackStart('foo', 3)).toBe(0);
    expect(wordBackStart('', 0)).toBe(0);
  });

  it('undo restores previous snapshot and can hit empty-stack floor', () => {
    let s = createComposerState();
    s = reduceComposer(s, { type: 'undo' }); // empty stack no-op
    expect(s.value).toBe('');
    s = reduceComposer(s, { type: 'insert', text: 'v1' });
    s = reduceComposer(s, { type: 'insert', text: '-v2' });
    s = reduceComposer(s, { type: 'undo' });
    expect(s.value).toBe('v1');
    s = reduceComposer(s, { type: 'undo' });
    expect(s.value).toBe('');
  });
});

describe('large-paste truncation', () => {
  it('passes small pastes through unchanged', () => {
    const r = processPaste('short', 0);
    expect(r.inserted).toBe('short');
    expect(r.stored).toBeUndefined();
    expect(r.nextCounter).toBe(0);
  });

  it('replaces huge pastes with a counted placeholder and stores full text', () => {
    // ~14k chars — comfortably over TRUNCATION_THRESHOLD (10_000).
    const big = Array.from({ length: 1600 }, (_, i) => `line-${i}`).join('\n');
    const r = processPaste(big, 3);
    expect(r.nextCounter).toBe(4);
    expect(r.stored?.id).toBe('paste_4');
    expect(r.stored?.content).toBe(big);
    expect(r.inserted).toBe(`[Pasted text #4 +${big.split('\n').length} lines]`);
    const round = expandPastedPlaceholders(`${r.inserted} tail`, { paste_4: big });
    expect(round).toBe(`${big} tail`);
  });

  it('expansion leaves unknown placeholders alone', () => {
    const v = '[Pasted text #9 +5 lines]';
    expect(expandPastedPlaceholders(v, {})).toBe(v);
  });
});

describe('history navigation', () => {
  it('up jumps to newest matching-mode entry, walks older, down restores draft', () => {
    let s = createComposerState(HIST);
    s = reduceComposer(s, { type: 'insert', text: 'dra' });
    s = reduceComposer(s, { type: 'history_up' });
    expect(s.value).toBe('second prompt'); // newest prompt-mode entry
    expect(s.historyDraft).toBe('dra');
    s = reduceComposer(s, { type: 'history_up' });
    expect(s.value).toBe('first prompt');
    s = reduceComposer(s, { type: 'history_up' });
    expect(s.value).toBe('first prompt'); // oldest prompt entry clamps
    s = reduceComposer(s, { type: 'history_down' });
    expect(s.value).toBe('second prompt');
    s = reduceComposer(s, { type: 'history_down' });
    expect(s.value).toBe('dra'); // draft restored, index back to -1
    expect(s.historyIndex).toBe(-1);
  });

  it('filters by browse mode captured at start (bang switches to bash entries)', () => {
    let s = createComposerState(HIST);
    s = reduceComposer(s, { type: 'insert', text: '!' });
    s = reduceComposer(s, { type: 'history_up' });
    // History stores bare submitted values; browsing restores them bare.
    // The captured browse mode keeps subsequent Up/Down in bash entries.
    expect(s.value).toBe('ls -la');
    expect(s.historyBrowseMode).toBe('bash');
  });

  it('submit_done resets the buffer and records newest-first without dupes', () => {
    let s = createComposerState(HIST);
    s = reduceComposer(s, { type: 'insert', text: 'brand new' });
    s = reduceComposer(s, { type: 'submit_done', entry: { mode: 'prompt', value: 'brand new' } });
    expect(s.value).toBe('');
    expect(s.cursor).toBe(0);
    expect(s.historyIndex).toBe(-1);
    expect(s.history[0]).toEqual({ mode: 'prompt', value: 'brand new' });
    s = reduceComposer(s, { type: 'submit_done', entry: { mode: 'prompt', value: 'brand new' } });
    expect(s.history.filter((e) => e.value === 'brand new')).toHaveLength(1);
  });
});

describe('key translation', () => {
  it('maps navigation/editing keys to commands', () => {
    expect(translateKey('', { upArrow: true })).toEqual({ type: 'history_up' });
    expect(translateKey('', { downArrow: true })).toEqual({ type: 'history_down' });
    expect(translateKey('', { leftArrow: true })).toEqual({ type: 'left' });
    expect(translateKey('', { rightArrow: true })).toEqual({ type: 'right' });
    expect(translateKey('', { return: true })).toEqual({ type: 'submit' });
    expect(translateKey('', { return: true, shift: true })).toEqual({ type: 'newline' });
    expect(translateKey('', { backspace: true })).toEqual({ type: 'backspace' });
    expect(translateKey('', { delete: true })).toEqual({ type: 'backspace' });
    expect(translateKey('a', {})).toEqual({ type: 'insert', text: 'a' });
    expect(translateKey('a', { ctrl: true })).toEqual({ type: 'home' });
    expect(translateKey('e', { ctrl: true })).toEqual({ type: 'end' });
    expect(translateKey('k', { ctrl: true })).toEqual({ type: 'kill_to_end' });
    expect(translateKey('u', { ctrl: true })).toEqual({ type: 'kill_to_start' });
    expect(translateKey('w', { ctrl: true })).toEqual({ type: 'delete_word_back' });
    expect(translateKey('z', { ctrl: true })).toEqual({ type: 'undo' });
    expect(translateKey('_', { ctrl: true })).toEqual({ type: 'undo' });
    expect(translateKey('', { escape: true })).toEqual({ type: 'abort' });
    expect(translateKey('c', { ctrl: true })).toEqual({ type: 'exit' });
  });

  it('expandedValue joins stored pastes into submitted text', () => {
    const s = createComposerState();
    const p = processPaste('BIG\nTEXT', 0);
    let t = reduceComposer(s, { type: 'insert', text: `${p.inserted}!` });
    t = {
      ...t,
      pastedContents: { ...(t.pastedContents ?? {}), ...(p.stored ? { [p.stored.id]: p.stored.content } : {}) },
    };
    expect(expandedValue(t)).toBe('BIG\nTEXT!');
  });
});
