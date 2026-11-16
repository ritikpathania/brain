import { Box, Text } from 'ink';

import type { BrainTheme } from '../theme.js';

export function UserMessage(props: Readonly<{ text: string; theme: BrainTheme }>) {
  return (
    <Box flexDirection="column" marginBottom={1}>
      <Text color={props.theme.accent}>You</Text>
      <Text>{props.text}</Text>
    </Box>
  );
}
