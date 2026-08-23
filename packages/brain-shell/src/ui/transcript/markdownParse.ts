/**
 * Terminal markdown subset renderer (pure): source → styled segments.
 * Deliberately small: headings, fenced code, lists, bold/italic/code/links.
 * Anything unrecognized passes through as plain text; parsing never throws.
 */

export type MdStyle =
  | 'plain'
  | 'bold'
  | 'italic'
  | 'code'
  | 'codeBlock'
  | 'header'
  | 'bulletMarker'
  | 'linkText'
  | 'linkUrl';

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

export function parseMarkdown(source: string): MdLine[] {
  const out: MdLine[] = [];
  let inFence = false;
  for (const raw of source.split('\n')) {
    if (/^```/.test(raw.trim())) {
      inFence = !inFence;
      continue;
    }
    if (inFence) {
      out.push({ segments: [{ text: raw.length > 0 ? raw : ' ', style: 'codeBlock' }] });
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
