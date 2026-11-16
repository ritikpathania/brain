import { Box, Text } from 'ink';
import type { BrainTheme } from '../theme.js';

export function ContextView(props: Readonly<{ summary?: Readonly<{ retrievalCount: number; contextTokenCount: number }>; theme: BrainTheme }>) {
  const count = props.summary?.retrievalCount;
  const text = count === undefined ? 'No retrieved context for this session.'
    : `Latest response used ${count} retrieved source${count === 1 ? '' : 's'} · ${props.summary!.contextTokenCount} context tokens`;
  return <Box flexDirection="column" marginTop={1} flexShrink={0}>
    <Text color={props.theme.text} wrap="truncate">{text}</Text>
    <Text color={props.theme.muted} wrap="truncate">Enter / Esc to return</Text>
  </Box>;
}
