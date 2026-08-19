import figures from 'figures';
import * as React from 'react';
import { useContext } from 'react';
import { useQueuedMessage } from '../../vendor/claude/context/QueuedMessageContext.js';
import { Box, Text } from '../../vendor/claude/ink.js';
import { formatBriefTimestamp } from '../../vendor/claude/utils/formatBriefTimestamp.js';
import { findThinkingTriggerPositions, isUltrathinkEnabled } from '../../vendor/claude/utils/thinking.js';
import { MessageActionsSelectedContext } from '../../vendor/claude/components/messageActions.js';

type Props = {
  text: string;
  useBriefLayout?: boolean;
  timestamp?: string;
};

export function HighlightedThinkingText({
  text,
  useBriefLayout,
  timestamp,
}: Props): React.ReactNode {
  const isQueued = useQueuedMessage()?.isQueued ?? false;
  const isSelected = useContext(MessageActionsSelectedContext);
  const pointerColor = isSelected ? 'suggestion' : 'subtle';

  if (useBriefLayout) {
    const ts = timestamp ? formatBriefTimestamp(timestamp) : '';
    const labelColor = isQueued ? 'subtle' : 'briefLabelYou';
    const youText = <Text color={labelColor}>You</Text>;
    const tsText = ts ? <Text dimColor> {ts}</Text> : null;
    const header = (
      <Box flexDirection="row">
        {youText}
        {tsText}
      </Box>
    );
    const contentColor = isQueued ? 'subtle' : 'text';
    const contentText = <Text color={contentColor}>{text}</Text>;
    return (
      <Box flexDirection="column" paddingLeft={2}>
        {header}
        {contentText}
      </Box>
    );
  }

  const triggers = isUltrathinkEnabled() ? findThinkingTriggerPositions(text) : [];
  if (triggers.length === 0) {
    return (
      <Text>
        <Text color={pointerColor}>{figures.pointer} </Text>
        <Text color="text">{text}</Text>
      </Text>
    );
  }

  const parts: React.ReactNode[] = [];
  let cursor = 0;
  for (let i = 0; i < triggers.length; i++) {
    const t = triggers[i]!;
    if (t.start > cursor) {
      parts.push(
        <Text key={`plain-${cursor}`} color="text">
          {text.slice(cursor, t.start)}
        </Text>
      );
    }
    parts.push(
      <Text key={`trigger-${t.start}`} color="suggestion">
        {text.slice(t.start, t.end)}
      </Text>
    );
    cursor = t.end;
  }
  if (cursor < text.length) {
    parts.push(
      <Text key={`plain-${cursor}`} color="text">
        {text.slice(cursor)}
      </Text>
    );
  }

  return (
    <Text>
      <Text color={pointerColor}>{figures.pointer}</Text>
      {' '}
      {parts}
    </Text>
  );
}
