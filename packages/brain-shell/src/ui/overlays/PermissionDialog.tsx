/** Modal permission dialog: tool + summarized input + Allow/Deny/Always allow. */
import * as React from 'react';
import { Box, Text } from '../../compat/index.js';
import type { BrainTokens } from '../../state/palettes.js';
import type { PendingPermissionView } from '../../state/sessionController.js';
import { summarizeToolInput } from '../transcript/MessageRow.js';

export function PermissionDialogView(props: {
  req: PendingPermissionView;
  selected: number;
  tokens: BrainTokens;
}): React.ReactElement {
  const summary = summarizeToolInput(props.req.input);
  const opt = (label: string, i: number): string =>
    `${i === props.selected ? '❯ ' : '  '}[ ${label} ]`;
  return (
    <Box flexDirection="column" borderStyle="round" borderColor={props.tokens.warning} paddingX={1}>
      <Text bold color={props.tokens.warning}>Permission required</Text>
      <Text>
        {props.req.toolName}
        {summary.length > 0 ? ` — ${summary}` : ''}
      </Text>
      {props.req.reason ? <Text dimColor>{props.req.reason}</Text> : null}
      <Text>
        {opt('Allow', 0)}   {opt('Deny', 1)}   {opt('Always allow', 2)}
      </Text>
      <Text dimColor>←→ choose · enter confirm · y allow · a always · n deny · esc denies</Text>
    </Box>
  );
}
