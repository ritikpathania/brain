import { Box, Text } from 'ink';

import type { TerminalEntry } from '../models.js';
import type { BrainTheme } from '../theme.js';
import { AssistantMessage } from './assistantMessage.js';
import { ErrorView } from './errorView.js';
import { Thinking } from './thinking.js';
import { UserMessage } from './userMessage.js';
import { Tool } from './tool.js';

export function Transcript(props: Readonly<{ entries: readonly TerminalEntry[]; theme: BrainTheme }>) {
  if (props.entries.length === 0) {
    return <Text color={props.theme.muted}>Start a conversation with Brain.</Text>;
  }
  return (
    <Box flexDirection="column" flexShrink={0}>
      {props.entries.map(entry => <TranscriptEntry key={entry.id} entry={entry} theme={props.theme} />)}
    </Box>
  );
}

function TranscriptEntry(props: Readonly<{ entry: TerminalEntry; theme: BrainTheme }>) {
  switch (props.entry.kind) {
    case 'user':
      return <UserMessage text={props.entry.text} theme={props.theme} />;
    case 'assistant':
      return <AssistantMessage text={props.entry.text} status={props.entry.status} theme={props.theme} />;
    case 'thinking':
      return <Thinking text={props.entry.text} status={props.entry.status} theme={props.theme} />;
    case 'tool':
      return <Tool tool={props.entry} theme={props.theme} />;
    case 'permission':
      return <Text color={props.theme.warning} wrap="truncate">{props.entry.status === 'allow_submitted' ? 'Allow submitted · awaiting Brain' : props.entry.status === 'deny_submitted' ? 'Deny submitted · awaiting Brain' : props.entry.status === 'failed' ? 'Permission decision unconfirmed' : 'Permission required'}: {props.entry.label}</Text>;
    case 'error':
      return <ErrorView message={props.entry.message} theme={props.theme} />;
    case 'status':
      return <Text color={props.theme.muted}>{props.entry.message}</Text>;
  }
}
