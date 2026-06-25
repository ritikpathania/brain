import { expect, test, describe } from 'bun:test';
import { parseMarkdown, parseInline } from '../MarkdownParser';

describe('MarkdownParser AST generation', () => {
  test('parses plain text and inline elements', () => {
    const inlineText = 'This is **bold** and *italic* text with `code` inline.';
    const parsed = parseInline(inlineText);

    expect(parsed).toEqual([
      { type: 'text', value: 'This is ' },
      { type: 'bold', text: 'bold' },
      { type: 'text', value: ' and ' },
      { type: 'italic', text: 'italic' },
      { type: 'text', value: ' text with ' },
      { type: 'inlinecode', code: 'code' },
      { type: 'text', value: ' inline.' }
    ]);
  });

  test('parses headings', () => {
    const md = '# Heading 1\n## Heading 2\n### Heading 3';
    const parsed = parseMarkdown(md);

    expect(parsed).toEqual([
      { type: 'heading', level: 1, children: [{ type: 'text', value: 'Heading 1' }] },
      { type: 'heading', level: 2, children: [{ type: 'text', value: 'Heading 2' }] },
      { type: 'heading', level: 3, children: [{ type: 'text', value: 'Heading 3' }] }
    ]);
  });

  test('parses lists (ordered and unordered)', () => {
    const unorderedMd = '- Item 1\n* Item 2\n• Item 3';
    const unorderedParsed = parseMarkdown(unorderedMd);

    expect(unorderedParsed).toEqual([
      {
        type: 'list',
        ordered: false,
        children: [
          { type: 'listitem', children: [{ type: 'text', value: 'Item 1' }] },
          { type: 'listitem', children: [{ type: 'text', value: 'Item 2' }] },
          { type: 'listitem', children: [{ type: 'text', value: 'Item 3' }] }
        ]
      }
    ]);

    const orderedMd = '1. First\n2. Second';
    const orderedParsed = parseMarkdown(orderedMd);

    expect(orderedParsed).toEqual([
      {
        type: 'list',
        ordered: true,
        children: [
          { type: 'listitem', children: [{ type: 'text', value: 'First' }] },
          { type: 'listitem', children: [{ type: 'text', value: 'Second' }] }
        ]
      }
    ]);
  });

  test('parses code blocks', () => {
    const md = '```typescript\nconst a = 123;\nconsole.log(a);\n```';
    const parsed = parseMarkdown(md);

    expect(parsed).toEqual([
      {
        type: 'codeblock',
        lang: 'typescript',
        code: 'const a = 123;\nconsole.log(a);'
      }
    ]);
  });

  test('parses horizontal rules', () => {
    const md = '---\n***\n___';
    const parsed = parseMarkdown(md);

    expect(parsed).toEqual([
      { type: 'horizontalrule' },
      { type: 'horizontalrule' },
      { type: 'horizontalrule' }
    ]);
  });

  test('parses quotes', () => {
    const md = '> Quote line 1\n> Quote line 2';
    const parsed = parseMarkdown(md);

    expect(parsed).toEqual([
      {
        type: 'quote',
        children: [
          {
            type: 'paragraph',
            children: [{ type: 'text', value: 'Quote line 1 Quote line 2' }]
          }
        ]
      }
    ]);
  });

  test('parses complex paragraph combinations', () => {
    const md = 'Paragraph 1\n\nParagraph 2 with **bold** text';
    const parsed = parseMarkdown(md);

    expect(parsed).toEqual([
      {
        type: 'paragraph',
        children: [{ type: 'text', value: 'Paragraph 1' }]
      },
      {
        type: 'paragraph',
        children: [
          { type: 'text', value: 'Paragraph 2 with ' },
          { type: 'bold', text: 'bold' },
          { type: 'text', value: ' text' }
        ]
      }
    ]);
  });
});
