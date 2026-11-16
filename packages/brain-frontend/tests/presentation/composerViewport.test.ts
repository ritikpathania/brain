import { expect, test } from 'bun:test';
import stringWidth from 'string-width';

import { composerViewport } from '../../src/presentation/input/composerViewport.js';

test('composer viewport wraps wide characters without changing the draft and bounds physical rows', () => {
  const draft = `${'🙂界'.repeat(20)}end`;
  const lines = composerViewport(draft, draft.length, 12, 3);
  expect(lines).toHaveLength(3);
  expect(lines.map(line => `${line.before}${line.cursor ?? ''}${line.after}`).join('')).toContain('end');
  for (const line of lines) expect(stringWidth(`${line.before}${line.cursor ?? ''}${line.after}`)).toBeLessThanOrEqual(12);
  expect(draft).toBe(`${'🙂界'.repeat(20)}end`);
});

test('composer viewport keeps a cursor on an empty multiline boundary visible', () => {
  const draft = 'first\n\nlast';
  const lines = composerViewport(draft, 6, 20);
  expect(lines).toEqual([
    { before: 'first', after: '' },
    { before: '', cursor: ' ', after: '' },
    { before: 'last', after: '' },
  ]);
});
