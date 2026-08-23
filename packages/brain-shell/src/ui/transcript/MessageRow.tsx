import * as React from 'react';
import { Box, Text, useTheme } from '../../compat/index.js';
import type { BrainTokens } from '../../state/palettes.js';
import type { TranscriptRow, ToolCardData } from '../../contracts/messages.js';
import { Markdown } from './Markdown.js';

export function UserRowView(props: {
  row: Extract<TranscriptRow, { kind: 'user' }>;
  tokens: BrainTokens;
}): React.ReactElement {
  return (
    <Text>
      <Text color={props.tokens.brand}>❯ </Text>
      {props.row.text}
    </Text>
  );
}

export function AssistantRowView(props: {
  row: Extract<TranscriptRow, { kind: 'assistant' }>;
}): React.ReactElement {
  return <Markdown source={props.row.markdown} />;
}

export function ThinkingRowView(props: {
  row: Extract<TranscriptRow, { kind: 'thinking' }>;
  tokens: BrainTokens;
}): React.ReactElement {
  const { row, tokens } = props;
  return (
    <Box flexDirection="column">
      {row.durationMs !== undefined ? (
        <Text dimColor>✻ Thought for {(row.durationMs / 1000).toFixed(1)}s</Text>
      ) : null}
      <Text dimColor italic color={tokens.subtle}>
        {'✻ '}
        {row.text}
      </Text>
    </Box>
  );
}

export function summarizeToolInput(input: Record<string, unknown>): string {
  const preferred = ['command', 'file_path', 'path', 'query', 'pattern', 'url', 'prompt'];
  for (const key of preferred) {
    const v = input[key];
    if (typeof v === 'string' && v.trim().length > 0) return v.trim().slice(0, 60);
  }
  for (const v of Object.values(input)) {
    if (typeof v === 'string' && v.trim().length > 0) return v.trim().slice(0, 60);
  }
  return '';
}

function statusMeta(
  status: ToolCardData['status'],
  durationMs: number | undefined,
): { glyph: string; label: string } {
  switch (status) {
    case 'pending':
    case 'running':
      return { glyph: '⏳', label: 'Running…' };
    case 'completed':
      return {
        glyph: '✓',
        label: durationMs !== undefined ? `Done in ${(durationMs / 1000).toFixed(1)}s` : 'Done',
      };
    case 'failed':
      return { glyph: '✗', label: 'Failed' };
    case 'denied':
      return { glyph: '✗', label: 'Permission denied' };
    case 'cancelled':
      return { glyph: '⏹', label: 'Cancelled' };
  }
}

export function ToolRowView(props: {
  row: Extract<TranscriptRow, { kind: 'tool' }>;
  expanded: boolean;
  tokens: BrainTokens;
}): React.ReactElement {
  const { row, expanded, tokens } = props;
  const t = row.tool;
  const meta = statusMeta(t.status, t.durationMs);
  const summary = summarizeToolInput(t.input);
  const statusColor =
    t.status === 'completed'
      ? tokens.success
      : t.status === 'failed' || t.status === 'denied' || t.status === 'cancelled'
        ? tokens.error
        : tokens.brand;
  return (
    <Box flexDirection="column">
      <Text>
        <Text color={tokens.brand}>⏺ </Text>
        <Text bold>{t.toolName}</Text>
        {summary.length > 0 ? <Text color={tokens.muted}>{`(${summary})`}</Text> : null}
      </Text>
      <Text>
        {'  '}
        <Text color={statusColor}>⎿ {meta.glyph}</Text>
        {expanded ? (
          <Text color={tokens.subtle}>
            {'\n     '}
            {JSON.stringify(t.input, null, 2)
              .split('\n')
              .join('\n     ')}
          </Text>
        ) : (
          <Text color={tokens.subtle}>{` ${meta.label}`}</Text>
        )}
      </Text>
    </Box>
  );
}

export function ErrorRowView(props: {
  row: Extract<TranscriptRow, { kind: 'error' }>;
  tokens: BrainTokens;
}): React.ReactElement {
  return (
    <Text>
      <Text color={props.tokens.warning}>⚠ </Text>
      <Text color={props.tokens.error}>{props.row.text}</Text>
    </Text>
  );
}

/**
 * Memoized dispatch: completed rows keep identity across snapshots, so frozen
 * rows skip re-render entirely; only `expanded` toggles recompute them.
 */
export const MessageRow = React.memo(
  function MessageRow(props: { row: TranscriptRow; expanded: boolean }): React.ReactElement {
    const { tokens } = useTheme();
    switch (props.row.kind) {
      case 'user':
        return <UserRowView row={props.row} tokens={tokens} />;
      case 'assistant':
        return <AssistantRowView row={props.row} />;
      case 'thinking':
        return <ThinkingRowView row={props.row} tokens={tokens} />;
      case 'tool':
        return <ToolRowView row={props.row} expanded={props.expanded} tokens={tokens} />;
      case 'error':
        return <ErrorRowView row={props.row} tokens={tokens} />;
    }
  },
  (a, b) => a.row === b.row && a.expanded === b.expanded,
);
