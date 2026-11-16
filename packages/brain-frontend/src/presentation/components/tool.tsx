import { Box, Text } from 'ink';
import type { TerminalEntry } from '../models.js';
import type { BrainTheme } from '../theme.js';

type ToolEntry = Extract<TerminalEntry, { kind: 'tool' }>;
const labels: Readonly<Record<ToolEntry['status'], string>> = Object.freeze({
  pending: 'pending', running: 'running', succeeded: 'succeeded', failed: 'failed',
  cancelled: 'cancelled', timed_out: 'timed out', interrupted: 'interrupted',
});

export function Tool(props: Readonly<{ tool: ToolEntry; theme: BrainTheme }>) {
  const { tool, theme } = props;
  const color = tool.status === 'failed' || tool.status === 'timed_out' ? theme.error : tool.status === 'succeeded' ? theme.success : theme.muted;
  return <Box flexDirection="column" flexShrink={0}>
    <Text color={color} wrap="truncate">Tool {labels[tool.status]}: {tool.label}{tool.action ? ` · ${tool.action}` : ''}{tool.summary ? ` · ${tool.summary}` : ''}</Text>
    {tool.detail && <Text color={theme.muted} wrap="truncate">{tool.expanded ? 'Ctrl+T to collapse output' : 'Ctrl+T to show output'}</Text>}
    {tool.expanded && tool.detail && <Text color={theme.text}>{tool.detail}</Text>}
  </Box>;
}
