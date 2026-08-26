/** Shared bordered frame for command overlays (/doctor, /memory). Pure view. */
import * as React from 'react';
import { Box, Text } from '../../compat/index.js';

export function ModalFrame(props: {
  title: string;
  subtitle?: string;
  footerHints?: string;
  width: number;
  children: React.ReactNode;
}): React.ReactElement {
  return (
    <Box flexDirection="column" borderStyle="round" width={props.width} paddingX={1}>
      <Text bold>{props.title}</Text>
      {props.subtitle ? <Text dimColor>{props.subtitle}</Text> : null}
      {props.children}
      {props.footerHints ? <Text dimColor>{props.footerHints}</Text> : null}
    </Box>
  );
}
