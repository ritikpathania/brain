import stringWidth from 'string-width';

export type ComposerViewportLine = Readonly<{ before: string; cursor?: string; after: string }>;

/** A bounded, cursor-visible projection; wrapping never edits the actual draft. */
export function composerViewport(draft: string, cursor: number, columns: number, maxLines = 4): readonly ComposerViewportLine[] {
  const width = Math.max(2, columns - 1); // Reserve a cell for a cursor at the end of a line.
  const rows: Array<{ text: string; start: number }> = [];
  let text = '';
  let start = 0;
  let cells = 0;
  for (const part of new Intl.Segmenter(undefined, { granularity: 'grapheme' }).segment(draft)) {
    if (part.segment === '\n') {
      rows.push({ text, start });
      text = '';
      start = part.index + 1;
      cells = 0;
      continue;
    }
    const nextCells = stringWidth(part.segment);
    if (text.length > 0 && cells + nextCells > width) {
      rows.push({ text, start });
      text = '';
      start = part.index;
      cells = 0;
    }
    text += part.segment;
    cells += nextCells;
  }
  rows.push({ text, start });
  let cursorRow = 0;
  for (let index = rows.length - 1; index >= 0; index -= 1) {
    if (rows[index]!.start <= cursor) { cursorRow = index; break; }
  }
  const first = Math.max(0, cursorRow - maxLines + 1);
  return Object.freeze(rows.slice(first, first + maxLines).map((row, index) => {
    if (first + index !== cursorRow) return Object.freeze({ before: row.text, after: '' });
    const offset = cursor - row.start;
    const current = Array.from(row.text.slice(offset))[0] ?? ' ';
    return Object.freeze({ before: row.text.slice(0, offset), cursor: current, after: row.text.slice(offset + current.length) });
  }));
}
