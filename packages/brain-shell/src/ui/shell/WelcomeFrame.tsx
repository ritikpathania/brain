/**
 * Launch-screen frame: wordmark, identity, workspace, and hint block.
 * Shown only while the transcript is empty; the conversation owns the
 * screen afterwards (replaces the Inc 0 BrainMark mount, per its comment).
 */
import * as React from 'react';
import * as path from 'path';
import { Box, Text, useTheme } from '../../compat/index.js';
import type { BrainTokens } from '../../state/palettes.js';

export function WelcomeFrameView(props: {
  tokens: BrainTokens;
  workspace: string;
}): React.ReactElement {
  return (
    <Box flexDirection="column" marginBottom={1}>
      <Text>
        <Text bold color={props.tokens.brand}>◆ BRAIN</Text>
        <Text dimColor> memory-first agent workspace</Text>
      </Text>
      <Box marginTop={1} flexDirection="column">
        <Text dimColor>  workspace {props.workspace}</Text>
        <Text dimColor>  /help commands · ! bash · /resume sessions · /theme appearance</Text>
      </Box>
    </Box>
  );
}

/** Hooked wrapper: theme tokens + cwd basename as the workspace label. */
export function WelcomeFrame(): React.ReactElement {
  const { tokens } = useTheme();
  const workspace = path.basename(process.cwd()).slice(0, 24);
  return <WelcomeFrameView tokens={tokens} workspace={workspace} />;
}
