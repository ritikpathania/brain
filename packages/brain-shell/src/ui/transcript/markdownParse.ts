/**
 * Terminal markdown subset renderer (pure): source → styled segments.
 * Deliberately small: headings, fenced code, lists, bold/italic/code/links.
 * Fenced blocks tagged with a supported language are syntax-highlighted via
 * syntax.ts; unknown or missing tags keep the whole-line codeBlock style.
 * Anything unrecognized passes through as plain text; parsing never throws.
 */

import { createCodeTokenizer } from './syntax.js';
import type { CodeTokenKind } from './syntax.js';

export type MdStyle =
  | 'plain'
  | 'bold'
  | 'italic'
  | 'code'
  | 'codeBlock'
  | 'header'
  | 'bulletMarker'
  | 'linkText'
  | 'linkUrl'
  | 'codeText'
  | 'codeKeyword'
  | 'codeString'
  | 'codeComment'
  | 'codeNumber'
  | 'codeFn';

export interface MdSegment {
  text: string;
  style: MdStyle;
}

export interface MdLine {
  segments: MdSegment[];
}

export function parseInline(text: string): MdSegment[] {
  if (text.length === 0) return [];
  const segs: MdSegment[] = [];
  const re =
    /(\*\*([^*]+)\*\*)|(`([^`]+)`)|(\*([^*]+)\*)|(\[([^\]]+)\]\(([^)]+)\))/g;
  let last = 0;
  let m: RegExpExecArray | null;
  while ((m = re.exec(text)) !== null) {
    if (m.index > last) segs.push({ text: text.slice(last, m.index), style: 'plain' });
    if (m[2] !== undefined) segs.push({ text: m[2], style: 'bold' });
    else if (m[4] !== undefined) segs.push({ text: m[4], style: 'code' });
    else if (m[6] !== undefined) segs.push({ text: m[6], style: 'italic' });
    else if (m[8] !== undefined) {
      segs.push({ text: m[8], style: 'linkText' });
      segs.push({ text: ` (${m[9]})`, style: 'linkUrl' });
    }
    last = re.lastIndex;
  }
  if (last < text.length) segs.push({ text: text.slice(last), style: 'plain' });
  return segs;
}

const HIGHLIGHT_STYLE: Record<CodeTokenKind, MdStyle> = {
  plain: 'codeText',
  keyword: 'codeKeyword',
  string: 'codeString',
  comment: 'codeComment',
  number: 'codeNumber',
  fn: 'codeFn',
};

export function parseMarkdown(source: string): MdLine[] {
  const out: MdLine[] = [];
  let inFence = false;
  let fenceTokens: ReturnType<typeof createCodeTokenizer> | undefined;
  for (const raw of source.split('\n')) {
    if (/^```/.test(raw.trim())) {
      if (!inFence) {
        inFence = true;
        const info = /^`+\s*([^\s`]*)/.exec(raw.trim())?.[1] ?? '';
        fenceTokens = createCodeTokenizer(info);
      } else {
        inFence = false;
        fenceTokens = undefined;
      }
      continue;
    }
    if (inFence) {
      const text = raw.length > 0 ? raw : ' ';
      if (fenceTokens) {
        out.push({
          segments: fenceTokens.line(text).map((t) => ({ text: t.text, style: HIGHLIGHT_STYLE[t.kind] })),
        });
      } else {
        out.push({ segments: [{ text, style: 'codeBlock' }] });
      }
      continue;
    }
    const header = /^#{1,6}\s+(.*)$/.exec(raw);
    if (header) {
      out.push({ segments: [{ text: header[1]!, style: 'header' }] });
      continue;
    }
    const bullet = /^(\s*)[-*]\s+(.*)$/.exec(raw);
    if (bullet) {
      out.push({
        segments: [{ text: `${bullet[1]}• `, style: 'bulletMarker' }, ...parseInline(bullet[2]!)],
      });
      continue;
    }
    const ordered = /^(\s*)\d+[.)]\s+(.*)$/.exec(raw);
    if (ordered) {
      out.push({
        segments: [{ text: `${ordered[1]}· `, style: 'bulletMarker' }, ...parseInline(ordered[2]!)],
      });
      continue;
    }
    out.push({ segments: parseInline(raw) });
  }
  return out;
}
