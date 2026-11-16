import { Box, Text } from 'ink';
import type { BrainTheme } from '../theme.js';

export function PermissionPrompt(props: Readonly<{ permission: Readonly<{ label: string; summary: string }>; theme: BrainTheme }>) {
  return <Box flexDirection="column" marginTop={1} flexShrink={0}>
    <Text color={props.theme.warning} wrap="truncate">Permission required: {props.permission.label}</Text>
    <Text color={props.theme.text} wrap="truncate">{props.permission.summary}</Text>
    <Text color={props.theme.muted} wrap="truncate">y allow · n / Esc deny · Ctrl+C cancel turn</Text>
  </Box>;
}
