import { Box, Text } from 'ink';

import type { BrainTheme } from '../theme.js';
import type { ComposerViewportLine } from '../input/composerViewport.js';

export function PromptComposer(props: Readonly<{
  lines: readonly ComposerViewportLine[];
  generating: boolean;
  cancelling: boolean;
  theme: BrainTheme;
}>) {
  const label = props.cancelling ? 'Cancelling…' : props.generating ? 'Generating…' : 'Message Brain';
  return (
    <Box flexDirection="column" marginTop={1}>
      <Text color={props.theme.accent} wrap="truncate">{label}</Text>
      {props.lines.map((line, index) => <Text key={index}>{index === 0 ? '› ' : '  '}{line.before}{line.cursor !== undefined && <Text inverse>{line.cursor}</Text>}{line.after}</Text>)}
      <Text color={props.theme.muted} wrap="truncate">Enter to send · Shift+Enter / Ctrl+J newline · Ctrl+O sessions</Text>
    </Box>
  );
}
