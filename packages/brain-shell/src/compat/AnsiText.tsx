/**
 * Minimal original ANSI renderer: maps SGR sequences onto Ink <Text> spans.
 * Supported SGR: 0 (reset), 1, 2, 3, 4, 22, 23, 24, 30-37, 39, 90-97.
 * Truecolor/256-color sequences are stripped (rendered as plain spans) —
 * full-fidelity passthrough arrives with the Inc 1 transcript renderer.
 */
import * as React from 'react';
import { Text } from 'ink';

interface SpanProps {
  bold?: boolean;
  dimColor?: boolean;
  italic?: boolean;
  underline?: boolean;
  color?: string;
}

const FG: Record<number, string> = {
  30: 'black', 31: 'red', 32: 'green', 33: 'yellow',
  34: 'blue', 35: 'magenta', 36: 'cyan', 37: 'white',
  90: 'gray', 91: 'redBright', 92: 'greenBright', 93: 'yellowBright',
  94: 'blueBright', 95: 'magentaBright', 96: 'cyanBright', 97: 'whiteBright',
};

function parse(input: string): Array<{ text: string; props: SpanProps }> {
  const spans: Array<{ text: string; props: SpanProps }> = [];
  let props: SpanProps = {};
  let buf = '';
  let i = 0;
  const flush = () => {
    if (buf.length > 0) spans.push({ text: buf, props });
    buf = '';
  };
  while (i < input.length) {
    if (input[i] === '\x1b' && input[i + 1] === '[') {
      const end = input.indexOf('m', i);
      if (end !== -1 && /^\[[0-9;]*$/.test(input.slice(i + 1, end))) {
        flush();
        for (const code of input.slice(i + 2, end).split(';')) {
          const n = Number(code || '0');
          if (n === 0) props = {};
          else if (n === 1) props.bold = true;
          else if (n === 2) props.dimColor = true;
          else if (n === 3) props.italic = true;
          else if (n === 4) props.underline = true;
          else if (n === 22) { props.bold = false; props.dimColor = false; }
          else if (n === 23) props.italic = false;
          else if (n === 24) props.underline = false;
          else if (n === 39) props.color = undefined;
          else if (FG[n]) props = { ...props, color: FG[n] };
          // 38;5;n / 48;… and other codes: intentionally ignored
        }
        i = end + 1;
        continue;
      }
    }
    buf += input[i];
    i++;
  }
  flush();
  return spans;
}

export function Ansi({ children }: { children: string }): React.ReactElement {
  const spans = React.useMemo(() => parse(children ?? ''), [children]);
  return (
    <>
      {spans.map((s, idx) => (
        <Text key={idx} {...s.props}>{s.text}</Text>
      ))}
    </>
  );
}
