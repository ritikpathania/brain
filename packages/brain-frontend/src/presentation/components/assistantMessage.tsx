import { Box, Text } from 'ink';

import type { BrainTheme } from '../theme.js';

export function AssistantMessage(props: Readonly<{
  text: string;
  status: 'streaming' | 'complete' | 'cancelled' | 'failed';
  theme: BrainTheme;
}>) {
  return (
    <Box flexDirection="column" marginBottom={1}>
      <Text color={props.theme.success}>Brain{props.status === 'streaming' ? ' · streaming' : ''}</Text>
      {props.text.length > 0 && <Text>{props.text}</Text>}
      {props.status === 'cancelled' && <Text color={props.theme.muted}>Generation cancelled</Text>}
      {props.status === 'failed' && <Text color={props.theme.error}>Generation failed</Text>}
    </Box>
  );
}
