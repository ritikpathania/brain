/**
 * Deterministic command argument parsing for /memory subcommands.
 */

export interface ParsedMemoryAdd {
  label: string;
  content: string;
}

export type ParseMemoryAddResult =
  | { ok: true; data: ParsedMemoryAdd }
  | { ok: false; error: string };

/**
 * Tokenize a raw argument string preserving single and double quoted substrings.
 */
export function tokenizeArgs(raw: string): string[] {
  const tokens: string[] = [];
  const regex = /"([^"\\]*(?:\\.[^"\\]*)*)"|'([^'\\]*(?:\\.[^'\\]*)*)'|(\S+)/g;
  let match: RegExpExecArray | null;
  while ((match = regex.exec(raw)) !== null) {
    if (match[1] !== undefined) {
      tokens.push(match[1].replace(/\\"/g, '"'));
    } else if (match[2] !== undefined) {
      tokens.push(match[2].replace(/\\'/g, "'"));
    } else if (match[3] !== undefined) {
      tokens.push(match[3]);
    }
  }
  return tokens;
}

/**
 * Parse arguments for `/memory add`.
 *
 * Supported formats:
 * 1. Canonical: `/memory add <label> --content <text>` (label and text may be multi-word or quoted).
 * 2. Positional:
 *    - Single word/token: label and content are both set to that token.
 *    - Quoted first token: first quoted token is label, remaining tokens are content.
 *    - Multiple tokens without --content: first token is label, rest joined as content.
 */
export function parseMemoryAddArgs(args: string[]): ParseMemoryAddResult {
  if (args.length === 0) {
    return { ok: false, error: 'Usage: /memory add <label> [--content <text>]' };
  }

  const contentIdx = args.indexOf('--content');
  if (contentIdx !== -1) {
    const labelParts = args.slice(0, contentIdx);
    const contentParts = args.slice(contentIdx + 1);
    const label = labelParts.join(' ').trim();
    const content = contentParts.join(' ').trim();

    if (!label) {
      return {
        ok: false,
        error: 'Memory node requires a non-empty label. Usage: /memory add <label> --content <text>',
      };
    }
    if (!content) {
      return {
        ok: false,
        error: 'Memory node requires non-empty content when --content is specified.',
      };
    }
    return { ok: true, data: { label, content } };
  }

  if (args.length === 1) {
    const single = args[0]!.trim();
    if (!single) {
      return { ok: false, error: 'Usage: /memory add <label> [--content <text>]' };
    }
    return { ok: true, data: { label: single, content: single } };
  }

  const label = args[0]!.trim();
  const content = args.slice(1).join(' ').trim();
  if (!label || !content) {
    return { ok: false, error: 'Usage: /memory add <label> [--content <text>]' };
  }

  return { ok: true, data: { label, content } };
}
