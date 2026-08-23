import * as React from 'react';
import { Box, Text, useTerminalSize } from '../../compat/index.js';
import { BrainMark } from './BrainMark.js';
import { useMainLoopModel } from '../../contracts/model.js';

export function AppSkeleton(): React.ReactElement {
  const { columns } = useTerminalSize();
  const model = useMainLoopModel(); // hoisted — hooks never inside JSX
  return (
    <Box flexDirection="column" width={columns} borderStyle="round">
      <BrainMark />
      <Box marginTop={1}>
        <Text>› </Text><Text dimColor>composer arrives in increment 1</Text>
      </Box>
      <Box marginTop={1}><Text dimColor>model: {model} · ctrl+c exit</Text></Box>
    </Box>
  );
}
