export type ASTNode =
  | { type: 'paragraph'; children: ASTNode[] }
  | { type: 'heading'; level: number; children: ASTNode[] }
  | { type: 'list'; ordered: boolean; children: ASTNode[] }
  | { type: 'listitem'; children: ASTNode[] }
  | { type: 'codeblock'; lang?: string; code: string }
  | { type: 'bold'; text: string }
  | { type: 'italic'; text: string }
  | { type: 'text'; value: string }
  | { type: 'inlinecode'; code: string }
  | { type: 'horizontalrule' }
  | { type: 'quote'; children: ASTNode[] };

export function parseInline(text: string): ASTNode[] {
  const nodes: ASTNode[] = [];
  let remaining = text;

  while (remaining.length > 0) {
    const boldMatch = remaining.match(/^([^\*`]*)\*\*([^*]+)\*\*(.*)$/);
    const codeMatch = remaining.match(/^([^`\*]*)`([^`]+)`(.*)$/);
    const italicMatch = remaining.match(/^([^\*`]*)\*([^*]+)\*(.*)$/);

    let firstMatch: { index: number; type: 'bold' | 'code' | 'italic'; match: RegExpMatchArray } | null = null;

    if (boldMatch && boldMatch.index !== undefined) {
      firstMatch = { index: boldMatch[1].length, type: 'bold', match: boldMatch };
    }
    if (codeMatch && codeMatch.index !== undefined) {
      const idx = codeMatch[1].length;
      if (!firstMatch || idx < firstMatch.index) {
        firstMatch = { index: idx, type: 'code', match: codeMatch };
      }
    }
    if (italicMatch && italicMatch.index !== undefined) {
      const idx = italicMatch[1].length;
      if (!firstMatch || idx < firstMatch.index) {
        firstMatch = { index: idx, type: 'italic', match: italicMatch };
      }
    }

    if (firstMatch) {
      const { type, match } = firstMatch;
      const prefix = match[1];
      if (prefix) {
        nodes.push({ type: 'text', value: prefix });
      }
      if (type === 'bold') {
        nodes.push({ type: 'bold', text: match[2] });
      } else if (type === 'code') {
        nodes.push({ type: 'inlinecode', code: match[2] });
      } else if (type === 'italic') {
        nodes.push({ type: 'italic', text: match[2] });
      }
      remaining = match[3];
    } else {
      nodes.push({ type: 'text', value: remaining });
      break;
    }
  }

  return nodes;
}

export function parseMarkdown(markdown: string): ASTNode[] {
  const nodes: ASTNode[] = [];
  const lines = markdown.split(/\r?\n/);
  let i = 0;

  while (i < lines.length) {
    const line = lines[i];
    const trimmed = line.trim();

    // 1. CodeBlock
    if (trimmed.startsWith('```')) {
      const lang = trimmed.slice(3).trim();
      let code = '';
      i++;
      while (i < lines.length) {
        if (lines[i].trim().startsWith('```')) {
          break;
        }
        code += lines[i] + '\n';
        i++;
      }
      nodes.push({ type: 'codeblock', lang: lang || undefined, code: code.trim() });
      i++;
      continue;
    }

    // 2. Horizontal Rule
    if (trimmed === '---' || trimmed === '***' || trimmed === '___') {
      nodes.push({ type: 'horizontalrule' });
      i++;
      continue;
    }

    // 3. Heading
    const headingMatch = line.match(/^(#{1,6})\s+(.*)$/);
    if (headingMatch) {
      const level = headingMatch[1].length;
      const content = headingMatch[2].trim();
      nodes.push({ type: 'heading', level, children: parseInline(content) });
      i++;
      continue;
    }

    // 4. Quote
    if (trimmed.startsWith('>')) {
      // Collect consecutive quote lines
      let quoteContent = '';
      while (i < lines.length && lines[i].trim().startsWith('>')) {
        const quoteLine = lines[i].trim().slice(1); // remove '>'
        quoteContent += (quoteLine.startsWith(' ') ? quoteLine.slice(1) : quoteLine) + '\n';
        i++;
      }
      nodes.push({ type: 'quote', children: parseMarkdown(quoteContent.trim()) });
      continue;
    }

    // 5. Lists
    const isUnordered = trimmed.startsWith('- ') || trimmed.startsWith('* ') || trimmed.startsWith('• ');
    const isOrdered = /^\d+\.\s+/.test(trimmed);
    if (isUnordered || isOrdered) {
      const ordered = isOrdered;
      const listItems: ASTNode[] = [];

      while (i < lines.length) {
        const itemTrimmed = lines[i].trim();
        const itemUnordered = itemTrimmed.startsWith('- ') || itemTrimmed.startsWith('* ') || itemTrimmed.startsWith('• ');
        const itemOrdered = /^\d+\.\s+/.test(itemTrimmed);

        if (!itemUnordered && !itemOrdered) {
          break;
        }

        let itemText = '';
        if (itemUnordered) {
          itemText = itemTrimmed.slice(2).trim();
        } else {
          const match = itemTrimmed.match(/^\d+\.\s+(.*)$/);
          itemText = match ? match[1].trim() : itemTrimmed;
        }

        listItems.push({ type: 'listitem', children: parseInline(itemText) });
        i++;
      }

      nodes.push({ type: 'list', ordered, children: listItems });
      continue;
    }

    // Empty lines are skipped
    if (trimmed === '') {
      i++;
      continue;
    }

    // 6. Paragraph (fallback)
    // Collect consecutive text lines
    let paragraphText = '';
    while (i < lines.length) {
      const pLine = lines[i].trim();
      if (
        pLine === '' ||
        pLine.startsWith('```') ||
        pLine.startsWith('---') ||
        pLine.startsWith('***') ||
        pLine.startsWith('___') ||
        pLine.match(/^(#{1,6})\s+(.*)$/) ||
        pLine.startsWith('>') ||
        pLine.startsWith('- ') ||
        pLine.startsWith('* ') ||
        pLine.startsWith('• ') ||
        /^\d+\.\s+/.test(pLine)
      ) {
        break;
      }
      paragraphText += (paragraphText ? ' ' : '') + pLine;
      i++;
    }

    if (paragraphText) {
      nodes.push({ type: 'paragraph', children: parseInline(paragraphText) });
    }
  }

  return nodes;
}
