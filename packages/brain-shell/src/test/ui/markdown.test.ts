import { describe, it, expect } from 'bun:test';
import { parseMarkdown, parseInline } from '../../ui/transcript/markdownParse.js';

describe('parseInline', () => {
  it('styles bold, italic, code spans', () => {
    const segs = parseInline('a **bold** b *it* c `code` d');
    expect(segs).toEqual([
      { text: 'a ', style: 'plain' },
      { text: 'bold', style: 'bold' },
      { text: ' b ', style: 'plain' },
      { text: 'it', style: 'italic' },
      { text: ' c ', style: 'plain' },
      { text: 'code', style: 'code' },
      { text: ' d', style: 'plain' },
    ]);
  });

  it('renders links as text plus dim url suffix', () => {
    const segs = parseInline('see [docs](https://brain.local/x) now');
    expect(segs).toEqual([
      { text: 'see ', style: 'plain' },
      { text: 'docs', style: 'linkText' },
      { text: ' (https://brain.local/x)', style: 'linkUrl' },
      { text: ' now', style: 'plain' },
    ]);
  });

  it('returns plain passthrough for unstyled text and never throws on oddities', () => {
    expect(parseInline('just words')).toEqual([{ text: 'just words', style: 'plain' }]);
    expect(parseInline('**unclosed')).toEqual([{ text: '**unclosed', style: 'plain' }]);
    expect(parseInline('')).toEqual([]);
  });
});

describe('parseMarkdown blocks', () => {
  it('headers become single bold segments regardless of level', () => {
    const lines = parseMarkdown('# Title\n### Sub');
    expect(lines[0]!.segments).toEqual([{ text: 'Title', style: 'header' }]);
    expect(lines[1]!.segments).toEqual([{ text: 'Sub', style: 'header' }]);
  });

  it('fenced code blocks mark every inner line codeBlock and hide fences', () => {
    const lines = parseMarkdown('before\n```ts\nconst x = 1;\nreturn x;\n```\nafter');
    expect(lines).toHaveLength(4);
    expect(lines[0]!.segments).toEqual([{ text: 'before', style: 'plain' }]);
    expect(lines[1]!.segments.map((s) => s.style)).toEqual(['codeBlock']);
    expect(lines[1]!.segments[0]!.text).toContain('const x = 1;');
    expect(lines[2]!.segments.map((s) => s.style)).toEqual(['codeBlock']);
    expect(lines[3]!.segments).toEqual([{ text: 'after', style: 'plain' }]);
  });

  it('bullets and ordered lists get dim markers and inline-parsed bodies', () => {
    const lines = parseMarkdown('- plain item\n* has **bold**\n1. numbered');
    expect(lines[0]!.segments[0]).toEqual({ text: '• ', style: 'bulletMarker' });
    expect(lines[0]!.segments[1]).toEqual({ text: 'plain item', style: 'plain' });
    expect(lines[1]!.segments.some((s) => s.style === 'bold')).toBe(true);
    expect(lines[2]!.segments[0]).toEqual({ text: '· ', style: 'bulletMarker' });
    expect(lines[2]!.segments[1]).toEqual({ text: 'numbered', style: 'plain' });
  });

  it('blank lines produce empty lines (spacing preserved)', () => {
    const lines = parseMarkdown('a\n\nb');
    expect(lines).toHaveLength(3);
    expect(lines[1]!.segments).toEqual([]);
  });
});
