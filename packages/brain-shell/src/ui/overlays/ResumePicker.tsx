/** /resume overlay: prior sessions, pinned first, relative ages. */
import * as React from 'react';
import { Box, Text } from '../../compat/index.js';
import type { BrainTokens } from '../../state/palettes.js';
import type { ResumeVM } from './resumePickerLogic.js';

export function ResumePickerView(props: {
  items: readonly ResumeVM[];
  selectedIndex: number;
  tokens: BrainTokens;
}): React.ReactElement {
  const sel = Math.min(props.selectedIndex, Math.max(0, props.items.length - 1));
  return (
    <Box flexDirection="column" borderStyle="round" borderColor={props.tokens.promptBorder} paddingX={1}>
      <Text bold>Resume session</Text>
      {props.items.map((it, i) => (
        <Text key={it.id} inverse={i === sel}>
          {(i === sel ? '❯ ' : '  ') + (it.pinned ? '★ ' : '')}
          {`${it.title.slice(0, 46)} — ${it.age}`}
        </Text>
      ))}
      <Text dimColor>↑↓ navigate · enter resume · esc cancel</Text>
    </Box>
  );
}
