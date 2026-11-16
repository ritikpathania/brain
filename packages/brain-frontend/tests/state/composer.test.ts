import { expect, test } from 'bun:test';
import { appendNewline, appendToDraft, deletePreviousCharacter, moveDraftCursor, replaceDraft, emptyComposerState } from '../../src/state/composer.js';

test('multiline editing keeps a cursor and inserts/deletes at its current position', () => {
  let draft = replaceDraft(emptyComposerState(), 'first\nsecond');
  draft = moveDraftCursor(draft, 'home');
  draft = appendToDraft(draft, 'new ');
  expect(draft.draft).toBe('first\nnew second');
  draft = moveDraftCursor(draft, 'up');
  draft = appendNewline(draft);
  expect(draft.draft).toBe('firs\nt\nnew second');
  draft = deletePreviousCharacter(draft);
  expect(draft.draft).toBe('first\nnew second');
});

test('cursor deletion preserves complete Unicode codepoints', () => {
  let draft = replaceDraft(emptyComposerState(), 'a😀b');
  draft = moveDraftCursor(draft, 'left');
  draft = deletePreviousCharacter(draft);
  expect(draft.draft).toBe('ab');
  expect(draft.cursor).toBe(1);
});
