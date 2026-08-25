/**
 * Brain-owned terminal syntax tokenizer (pure): fenced code lines → tokens.
 * Deliberately small, in the spirit of markdownParse.ts: five language
 * families share one first-match scanner; anything unrecognized yields no
 * tokenizer and callers keep plain rendering. Tokenizing never throws —
 * any anomaly degrades the whole line to a single plain token. Adjacent
 * same-kind tokens merge, so output strictly alternates kinds.
 */

export type CodeTokenKind = 'plain' | 'keyword' | 'string' | 'comment' | 'number' | 'fn';

export interface CodeToken {
  text: string;
  kind: CodeTokenKind;
}

interface StringRule {
  open: string;
  close: string;
  escapes: boolean;
}

/** Per-family scan rules; the scanner is generic over these. */
interface LangRules {
  /** Openers that comment out the rest of the line. */
  lineComments: string[];
  /** Open/close pairs that may span lines (kind `comment`). */
  blockComments: Array<StringRule>;
  /** Delimiters that may span lines (kind `string`): backticks, triple quotes. */
  multilineStrings: Array<StringRule>;
  /** Same-line delimiters; an unterminated one ends at end of line. */
  strings: Array<StringRule>;
  keywords: Set<string>;
  /** Recognize identifier-followed-by-`(` as a `fn` token. */
  calls: boolean;
  /** Rust-style lifetimes: `'a` is not a char literal. */
  lifetimeTick: boolean;
  word: RegExp;
}

const WORD_TS = /[A-Za-z_$][A-Za-z0-9_$]*/y;
const WORD_PLAIN = /[A-Za-z_][A-Za-z0-9_]*/y;

const NUMBER =
  /0[xX][0-9a-fA-F_]+|0[bB][01_]+|0[oO][0-7_]+|\d[\d_]*(?:\.[\d_]*)?(?:[eE][+-]?\d+)?(?:ull|ll|ul|l|u|i8|i16|i32|i64|isize|u8|u16|u32|u64|usize|f32|f64|f|n)?/y;

function kw(...words: string[]): Set<string> {
  return new Set(words);
}

const RULES: Record<string, LangRules> = {
  ts: {
    lineComments: ['//'],
    blockComments: [{ open: '/*', close: '*/', escapes: false }],
    multilineStrings: [{ open: '`', close: '`', escapes: true }],
    strings: [
      { open: "'", close: "'", escapes: true },
      { open: '"', close: '"', escapes: true },
    ],
    keywords: kw(
      'abstract', 'any', 'as', 'asserts', 'async', 'await', 'boolean', 'break', 'case',
      'catch', 'class', 'const', 'continue', 'debugger', 'declare', 'default', 'delete',
      'do', 'else', 'enum', 'export', 'extends', 'false', 'finally', 'for', 'from',
      'function', 'get', 'if', 'implements', 'import', 'in', 'infer', 'instanceof',
      'interface', 'is', 'keyof', 'let', 'namespace', 'never', 'new', 'null', 'number',
      'object', 'of', 'override', 'private', 'protected', 'public', 'readonly', 'return',
      'satisfies', 'set', 'static', 'string', 'super', 'switch', 'symbol', 'this', 'throw',
      'true', 'try', 'type', 'typeof', 'undefined', 'unknown', 'var', 'void', 'while',
      'yield',
    ),
    calls: true,
    lifetimeTick: false,
    word: WORD_TS,
  },
  json: {
    lineComments: [],
    blockComments: [],
    multilineStrings: [],
    strings: [{ open: '"', close: '"', escapes: true }],
    keywords: kw('true', 'false', 'null'),
    calls: false,
    lifetimeTick: false,
    word: WORD_PLAIN,
  },
  bash: {
    lineComments: ['#'],
    blockComments: [],
    multilineStrings: [],
    strings: [
      { open: '"', close: '"', escapes: true },
      { open: "'", close: "'", escapes: false },
    ],
    keywords: kw(
      'alias', 'case', 'coproc', 'declare', 'do', 'done', 'elif', 'else', 'esac', 'eval',
      'exec', 'exit', 'export', 'fi', 'for', 'function', 'if', 'in', 'local', 'readonly',
      'return', 'select', 'set', 'shift', 'source', 'then', 'time', 'trap', 'typeset',
      'unset', 'until', 'while',
    ),
    calls: false,
    lifetimeTick: false,
    word: WORD_PLAIN,
  },
  rust: {
    lineComments: ['//'],
    blockComments: [{ open: '/*', close: '*/', escapes: false }],
    multilineStrings: [],
    strings: [
      { open: '"', close: '"', escapes: true },
      { open: "'", close: "'", escapes: true },
    ],
    keywords: kw(
      'as', 'async', 'await', 'break', 'const', 'continue', 'crate', 'dyn', 'else',
      'enum', 'extern', 'false', 'fn', 'for', 'if', 'impl', 'in', 'let', 'loop', 'match',
      'mod', 'move', 'mut', 'pub', 'ref', 'return', 'self', 'Self', 'static', 'struct',
      'super', 'trait', 'true', 'type', 'union', 'unsafe', 'use', 'where', 'while',
    ),
    calls: true,
    lifetimeTick: true,
    word: WORD_PLAIN,
  },
  python: {
    lineComments: ['#'],
    blockComments: [],
    multilineStrings: [
      { open: '"""', close: '"""', escapes: true },
      { open: "'''", close: "'''", escapes: true },
    ],
    strings: [
      { open: "'", close: "'", escapes: true },
      { open: '"', close: '"', escapes: true },
    ],
    keywords: kw(
      'and', 'as', 'assert', 'async', 'await', 'break', 'case', 'class', 'cls',
      'continue', 'def', 'del', 'elif', 'else', 'except', 'False', 'finally', 'for',
      'from', 'global', 'if', 'import', 'in', 'is', 'lambda', 'match', 'None', 'nonlocal',
      'not', 'or', 'pass', 'raise', 'return', 'self', 'super', 'True', 'try', 'while',
      'with', 'yield',
    ),
    calls: true,
    lifetimeTick: false,
    word: WORD_PLAIN,
  },
};

const ALIASES: Record<string, string> = {
  ts: 'ts', tsx: 'ts', mjs: 'ts', cjs: 'ts', js: 'ts', jsx: 'ts',
  javascript: 'ts', typescript: 'ts',
  json: 'json', jsonc: 'json',
  bash: 'bash', sh: 'bash', shell: 'bash', zsh: 'bash',
  rs: 'rust', rust: 'rust',
  py: 'python', python: 'python', python3: 'python',
};

/** Stateful tokenizer for one fenced code block (state threads across lines). */
export interface CodeTokenizer {
  /** Tokenize one line; adjacent same-kind tokens are merged. */
  line(source: string): CodeToken[];
}

interface ActiveSpan {
  close: string;
  kind: CodeTokenKind;
}

class Scanner implements CodeTokenizer {
  private active: ActiveSpan | undefined;

  constructor(private readonly rules: LangRules) {}

  line(source: string): CodeToken[] {
    const out: CodeToken[] = [];
    try {
      let i = 0;
      if (this.active) i = this.consumeSpan(source, out);
      while (i < source.length) i = this.scanAt(source, i, out);
    } catch {
      return [{ text: source, kind: 'plain' }];
    }
    return out;
  }

  /** Continue an open multi-line construct; returns the resume index. */
  private consumeSpan(text: string, out: CodeToken[]): number {
    const span = this.active!;
    const end = text.indexOf(span.close);
    if (end === -1) {
      this.push(out, text, span.kind);
      return text.length; // stays open for the next line
    }
    this.push(out, text.slice(0, end + span.close.length), span.kind);
    this.active = undefined;
    return end + span.close.length;
  }

  private scanAt(text: string, i: number, out: CodeToken[]): number {
    // Whitespace run
    const ws = /^\s+/y;
    ws.lastIndex = i;
    const wm = ws.exec(text);
    if (wm) return this.emit(out, text, i, i + wm[0].length, 'plain');

    if (this.rules.lineComments.some((op) => text.startsWith(op, i))) {
      this.push(out, text.slice(i), 'comment');
      return text.length;
    }

    for (const rule of this.rules.blockComments) {
      if (!text.startsWith(rule.open, i)) continue;
      return this.enterSpan(text, i, rule.close, 'comment', out);
    }

    for (const rule of this.rules.multilineStrings) {
      if (!text.startsWith(rule.open, i)) continue;
      return this.enterSpan(text, i, rule.close, 'string', out);
    }

    if (this.rules.lifetimeTick && text[i] === "'" && !this.isCharLiteral(text, i)) {
      const life = /^[A-Za-z_]/.exec(text.slice(i + 1));
      if (life) return this.emit(out, text, i, this.wordEnd(text, i + 1), 'plain');
    }

    for (const rule of this.rules.strings) {
      if (!text.startsWith(rule.open, i)) continue;
      let j = i + rule.open.length;
      while (j < text.length) {
        if (rule.escapes && text[j] === '\\') {
          j += 2;
          continue;
        }
        if (text.startsWith(rule.close, j)) {
          j += rule.close.length;
          break;
        }
        j += 1;
      }
      return this.emit(out, text, i, Math.min(j, text.length), 'string');
    }

    NUMBER.lastIndex = i;
    const num = NUMBER.exec(text);
    if (num) return this.emit(out, text, i, NUMBER.lastIndex, 'number');

    this.rules.word.lastIndex = i;
    const id = this.rules.word.exec(text);
    if (id) {
      const end = this.rules.word.lastIndex;
      if (this.rules.keywords.has(id[0])) return this.emit(out, text, i, end, 'keyword');
      if (this.rules.calls && this.nextSignificant(text, end) === '(')
        return this.emit(out, text, i, end, 'fn');
      return this.emit(out, text, i, end, 'plain');
    }

    return this.emit(out, text, i, i + 1, 'plain'); // punctuation, operators, $ etc.
  }

  /** Open a spanning construct; consume to its closer or end of line. */
  private enterSpan(
    text: string,
    i: number,
    close: string,
    kind: CodeTokenKind,
    out: CodeToken[],
  ): number {
    const end = text.indexOf(close, i + close.length);
    if (end === -1) {
      this.active = { close, kind };
      this.push(out, text.slice(i), kind);
      return text.length;
    }
    const stop = end + close.length;
    this.push(out, text.slice(i, stop), kind);
    return stop;
  }

  private isCharLiteral(text: string, i: number): boolean {
    const re = /'(?:\\.|[^\\'])'/y;
    re.lastIndex = i;
    return re.exec(text) !== null;
  }

  private wordEnd(text: string, from: number): number {
    this.rules.word.lastIndex = from;
    this.rules.word.exec(text);
    return this.rules.word.lastIndex;
  }

  private nextSignificant(text: string, from: number): string {
    let k = from;
    while (k < text.length && (text[k] === ' ' || text[k] === '\t')) k += 1;
    return text[k] ?? '';
  }

  private emit(out: CodeToken[], text: string, start: number, stop: number, kind: CodeTokenKind): number {
    this.push(out, text.slice(start, stop), kind);
    return Math.max(stop, start + 1); // always make progress
  }

  private push(out: CodeToken[], text: string, kind: CodeTokenKind): void {
    const last = out[out.length - 1];
    if (last && last.kind === kind) last.text += text;
    else out.push({ text, kind });
  }
}

/** Tokenizer factory: alias-resolves the fence info tag; unknown → undefined. */
export function createCodeTokenizer(lang: string): CodeTokenizer | undefined {
  const key = ALIASES[lang.trim().toLowerCase()];
  const rules = key === undefined ? undefined : RULES[key];
  return rules === undefined ? undefined : new Scanner(rules);
}
