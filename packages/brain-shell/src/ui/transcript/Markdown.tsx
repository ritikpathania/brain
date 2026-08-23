import * as React from 'react';
import { Text, useTheme } from '../../compat/index.js';
import { parseMarkdown } from './markdown.js';
import type { MdLine, MdStyle } from './markdown.js';

function flagsFor(style: MdStyle): {
  bold?: boolean;
  italic?: boolean;
  underline?: boolean;
  dimColor?: boolean;
} {
  switch (style) {
    case 'bold':
      return { bold: true };
    case 'italic':
      return { italic: true };
    case 'header':
      return { bold: true };
    case 'bulletMarker':
      return { dimColor: true };
    case 'linkText':
      return { underline: true };
    case 'linkUrl':
      return { dimColor: true };
    default:
      return {};
  }
}

function colorFor(
  style: MdStyle,
  tokens: ReturnType<typeof useTheme>['tokens'],
): string | undefined {
  switch (style) {
    case 'header':
      return tokens.brand;
    case 'code':
      return tokens.accent;
    case 'codeBlock':
      return tokens.subtle;
    default:
      return undefined;
  }
}

/** Segment renderer bound to theme tokens. */
export function MarkdownView(props: { lines: MdLine[] }): React.ReactElement {
  const { tokens } = useTheme();
  return (
    <>
      {props.lines.map((line, li) => (
        <Text key={li}>
          {line.segments.length === 0
            ? ' '
            : line.segments.map((seg, si) => (
                <Text key={si} {...flagsFor(seg.style)} color={colorFor(seg.style, tokens)}>
                  {seg.text}
                </Text>
              ))}
          {'\n'}
        </Text>
      ))}
    </>
  );
}

export function Markdown(props: { source: string }): React.ReactElement {
  return <MarkdownView lines={parseMarkdown(props.source)} />;
}
