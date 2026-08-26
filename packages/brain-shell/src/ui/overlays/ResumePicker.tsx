/** /resume overlay: prior sessions, pinned first, relative ages, typed filter. */
import * as React from 'react';
import { Box, Text } from '../../compat/index.js';
import type { BrainTokens } from '../../state/palettes.js';
import type { ResumeVM } from './resumePickerLogic.js';

export function ResumePickerView(props: {
  items: readonly ResumeVM[];
  selectedIndex: number;
  tokens: BrainTokens;
  query?: string;
  currentSessionId?: string;
}): React.ReactElement {
  const sel = Math.min(props.selectedIndex, Math.max(0, props.items.length - 1));
  return (
    <Box flexDirection="column" borderStyle="round" borderColor={props.tokens.promptBorder} paddingX={1}>
      <Text bold>Resume session</Text>
      <Text>› {props.query ?? ''}▏</Text>
      {props.items.length === 0 ? (
        <Text dimColor>No sessions match.</Text>
      ) : (
        props.items.map((it, i) => (
          <Text key={it.id} inverse={i === sel}>
            {(i === sel ? '❯ ' : '  ') + (it.pinned ? '★ ' : '')}
            {it.id === props.currentSessionId ? <Text dimColor>● </Text> : null}
            {`${it.title.slice(0, 46)} — ${it.age}`}
          </Text>
        ))
      )}
      <Text dimColor>↑↓ navigate · enter resume · esc cancel · type to filter</Text>
    </Box>
  );
}
