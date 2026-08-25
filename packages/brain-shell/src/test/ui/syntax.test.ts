import { describe, it, expect } from 'bun:test';
import { createCodeTokenizer } from '../../ui/transcript/syntax.js';

/** Tokens must reproduce the line byte-for-byte — highlighting never edits code text. */
function joined(tokens: Array<{ text: string }>): string {
  return tokens.map((t) => t.text).join('');
}

describe('createCodeTokenizer registry', () => {
  it('maps language aliases onto five families and rejects unknown tags', () => {
    for (const lang of ['ts', 'tsx', 'js', 'jsx', 'typescript', 'javascript'])
      expect(createCodeTokenizer(lang)).toBeDefined();
    for (const lang of ['json', 'bash', 'sh', 'shell', 'zsh', 'rust', 'rs', 'python', 'py'])
      expect(createCodeTokenizer(lang)).toBeDefined();
    expect(createCodeTokenizer('cobol')).toBeUndefined();
    expect(createCodeTokenizer('')).toBeUndefined();
    expect(createCodeTokenizer('Rust')).toBeDefined(); // case-insensitive
  });
});

describe('ts family tokenizer', () => {
  it('colors keywords, function calls, numbers, strings, comments', () => {
    const ts = createCodeTokenizer('ts')!;
    const tokens = ts.line('const retries = maxRetries(); // clamp');
    expect(tokens).toEqual([
      { text: 'const', kind: 'keyword' },
      { text: ' retries = ', kind: 'plain' },
      { text: 'maxRetries', kind: 'fn' },
      { text: '(); ', kind: 'plain' },
      { text: '// clamp', kind: 'comment' },
    ]);
  });

  it('colors escaped strings, template literals, and hex numbers', () => {
    const ts = createCodeTokenizer('ts')!;
    const tokens = ts.line('msg = "hi\\nthere" + `t${x}` + 0xff;');
    expect(tokens).toEqual([
      { text: 'msg = ', kind: 'plain' },
      { text: '"hi\\nthere"', kind: 'string' },
      { text: ' + ', kind: 'plain' },
      { text: '`t${x}`', kind: 'string' },
      { text: ' + ', kind: 'plain' },
      { text: '0xff', kind: 'number' },
      { text: ';', kind: 'plain' },
    ]);
  });

  it('threads block comments across lines', () => {
    const ts = createCodeTokenizer('ts')!;
    expect(ts.line('/* start')).toEqual([{ text: '/* start', kind: 'comment' }]);
    expect(ts.line('still */ const x')).toEqual([
      { text: 'still */', kind: 'comment' },
      { text: ' ', kind: 'plain' },
      { text: 'const', kind: 'keyword' },
      { text: ' x', kind: 'plain' },
    ]);
  });

  it('ends unterminated strings at end of line without leaking state', () => {
    const ts = createCodeTokenizer('ts')!;
    expect(ts.line('s = "oops')).toEqual([
      { text: 's = ', kind: 'plain' },
      { text: '"oops', kind: 'string' },
    ]);
    expect(ts.line('next = 1;')).toEqual([
      { text: 'next = ', kind: 'plain' },
      { text: '1', kind: 'number' },
      { text: ';', kind: 'plain' },
    ]);
  });
});

describe('python tokenizer', () => {
  it('colors def, call names, and # comments', () => {
    const py = createCodeTokenizer('py')!;
    expect(py.line('def greet(name):  # wave')).toEqual([
      { text: 'def', kind: 'keyword' },
      { text: ' ', kind: 'plain' },
      { text: 'greet', kind: 'fn' },
      { text: '(name):  ', kind: 'plain' },
      { text: '# wave', kind: 'comment' },
    ]);
  });

  it('threads triple-quoted strings across lines as string state', () => {
    const py = createCodeTokenizer('py')!;
    expect(py.line('"""doc')).toEqual([{ text: '"""doc', kind: 'string' }]);
    expect(py.line('end""" x = 1')).toEqual([
      { text: 'end"""', kind: 'string' },
      { text: ' x = ', kind: 'plain' },
      { text: '1', kind: 'number' },
    ]);
  });
});

describe('rust tokenizer', () => {
  it('colors pub/fn keywords, call names, and numeric indexing', () => {
    const rs = createCodeTokenizer('rs')!;
    expect(rs.line('pub fn main() { let x = &arr[0]; }')).toEqual([
      { text: 'pub', kind: 'keyword' },
      { text: ' ', kind: 'plain' },
      { text: 'fn', kind: 'keyword' },
      { text: ' ', kind: 'plain' },
      { text: 'main', kind: 'fn' },
      { text: '() { ', kind: 'plain' },
      { text: 'let', kind: 'keyword' },
      { text: ' x = &arr[', kind: 'plain' },
      { text: '0', kind: 'number' },
      { text: ']; }', kind: 'plain' },
    ]);
  });

  it('keeps lifetimes plain while coloring char literals', () => {
    const rs = createCodeTokenizer('rs')!;
    const tokens = rs.line("let c: char = 'x'; let r: &'a str;");
    expect(tokens.some((t) => t.text === "'x'" && t.kind === 'string')).toBe(true);
    expect(tokens.some((t) => t.text.includes("'a") && t.kind === 'string')).toBe(false);
  });
});

describe('bash tokenizer', () => {
  it('colors keywords, quoted variables, and # comments', () => {
    const sh = createCodeTokenizer('bash')!;
    expect(sh.line('if [ -f "$file" ]; then  # check')).toEqual([
      { text: 'if', kind: 'keyword' },
      { text: ' [ -f ', kind: 'plain' },
      { text: '"$file"', kind: 'string' },
      { text: ' ]; ', kind: 'plain' },
      { text: 'then', kind: 'keyword' },
      { text: '  ', kind: 'plain' },
      { text: '# check', kind: 'comment' },
    ]);
  });
});

describe('json tokenizer', () => {
  it('colors keys/values as strings, numbers, and literals', () => {
    const js = createCodeTokenizer('json')!;
    expect(js.line('{"key": [1, 2.5e3, true], "s": "v"}')).toEqual([
      { text: '{', kind: 'plain' },
      { text: '"key"', kind: 'string' },
      { text: ': [', kind: 'plain' },
      { text: '1', kind: 'number' },
      { text: ', ', kind: 'plain' },
      { text: '2.5e3', kind: 'number' },
      { text: ', ', kind: 'plain' },
      { text: 'true', kind: 'keyword' },
      { text: '], ', kind: 'plain' },
      { text: '"s"', kind: 'string' },
      { text: ': ', kind: 'plain' },
      { text: '"v"', kind: 'string' },
      { text: '}', kind: 'plain' },
    ]);
  });
});

describe('tokenizer robustness', () => {
  it('preserves text exactly for arbitrary odd input and never throws', () => {
    for (const lang of ['ts', 'py', 'rs', 'bash', 'json']) {
      const tok = createCodeTokenizer(lang)!;
      for (const line of [
        '"',
        '`',
        '\\',
        '/* /* /*',
        'héllo → wörld "mixed"',
        "''' \"\"\" //",
        '$((1+2)) ${x:-y}',
        '',
      ]) {
        let tokens;
        expect(() => (tokens = tok.line(line))).not.toThrow();
        if (line.length === 0) continue;
        expect(joined(tokens!)).toBe(line);
      }
    }
  });
});
