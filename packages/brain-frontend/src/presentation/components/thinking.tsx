import { Box, Text } from 'ink';

import type { BrainTheme } from '../theme.js';

export function Thinking(props: Readonly<{ text: string; status: 'streaming' | 'complete'; theme: BrainTheme }>) {
  return (
    <Box flexDirection="column" marginBottom={1}>
      <Text color={props.theme.muted}>Thinking{props.status === 'streaming' ? '…' : ''}</Text>
      <Text color={props.theme.muted}>{props.text}</Text>
    </Box>
  );
}
