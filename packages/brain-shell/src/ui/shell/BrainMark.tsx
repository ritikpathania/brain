import * as React from 'react';
import { Box, Text, useTheme } from '../../compat/ink.js';
import type { BrainTokens } from '../../state/palettes.js';

/** Pure view so tests can assert output without mounting a reconciler. */
export function BrainMarkView({ tokens }: { tokens: BrainTokens }): React.ReactElement {
  return (
    <Box flexDirection="column">
      <Text bold color={tokens.brand}>◆ BRAIN</Text>
      <Text dimColor>memory-first agent workspace</Text>
    </Box>
  );
}

/** Brain's launch mark: wordmark + one-line identity. Replaced by the full welcome frame in Inc 3. */
export function BrainMark(): React.ReactElement {
  const { tokens } = useTheme();
  return <BrainMarkView tokens={tokens} />;
}
