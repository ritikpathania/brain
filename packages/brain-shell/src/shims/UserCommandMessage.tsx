import figures from 'figures';
import * as React from 'react';
import { Box, Text } from '../compat/ink.js';
import type { TextBlock } from '../contracts/messages.js';
import { extractTag } from '../contracts/messages.js';

/** Tag wrapping slash commands echoed back inside user message text. */
const COMMAND_MESSAGE_TAG = 'command-message';

type Props = {
  addMargin: boolean;
  param: TextBlock;
};

export function UserCommandMessage({ addMargin, param: { text } }: Props): React.ReactNode {
  const commandMessage = extractTag(text, COMMAND_MESSAGE_TAG);
  const args = extractTag(text, 'command-args');
  const isSkillFormat = extractTag(text, 'skill-format') === 'true';

  if (!commandMessage) {
    return null;
  }

  if (isSkillFormat) {
    return (
      <Box
        flexDirection="column"
        marginTop={addMargin ? 1 : 0}
        backgroundColor="userMessageBackground"
        paddingRight={1}
      >
        <Text>
          <Text aria-label="you:" color="subtle">
            {figures.pointer}
          </Text>{' '}
          <Text color="text">Skill({commandMessage})</Text>
        </Text>
      </Box>
    );
  }

  const parts = [commandMessage, args].filter(Boolean);
  const content = `/${parts.join(' ')}`;

  return (
    <Box
      flexDirection="column"
      marginTop={addMargin ? 1 : 0}
      backgroundColor="userMessageBackground"
      paddingRight={1}
    >
      <Text>
        <Text aria-label="you:" color="subtle">
          {figures.pointer}
        </Text>{' '}
        <Text color="text">{content}</Text>
      </Text>
    </Box>
  );
}
