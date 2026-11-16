import { Box, Text } from 'ink';

import type { TerminalConnectionState } from '../models.js';
import type { BrainTheme } from '../theme.js';

const connectionLabel: Readonly<Record<TerminalConnectionState, string>> = Object.freeze({
  checking: 'Connecting',
  ready: 'Connected',
  disconnected: 'Disconnected',
  reconnecting: 'Reconnecting',
  incompatible: 'Incompatible daemon',
  failed: 'Connection failed',
});

export function StatusLine(props: Readonly<{
  connection: TerminalConnectionState;
  sessionTitle?: string;
  generating: boolean;
  cancelling: boolean;
  theme: BrainTheme;
}>) {
  const activity = props.cancelling ? ' · cancelling' : props.generating ? ' · generating' : '';
  const session = props.sessionTitle === undefined ? '' : ` · ${props.sessionTitle}`;
  return (
    <Box marginTop={1}>
      <Text color={props.theme.muted} wrap="truncate">{connectionLabel[props.connection]}{session}{activity}</Text>
    </Box>
  );
}
