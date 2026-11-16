import { Box, Text } from 'ink';

import type { BrainTheme } from '../theme.js';

export type SessionPickerItem = Readonly<{ sessionId: string; title: string; messageCount: number }>;

export function SessionPicker(props: Readonly<{
  sessions: readonly SessionPickerItem[];
  selectedIndex: number;
  visibleRows?: number;
  theme: BrainTheme;
}>) {
  const options = [
    { key: 'new', label: 'New conversation' },
    ...props.sessions.map(session => ({ key: session.sessionId, label: `${session.title} · ${session.messageCount} messages` })),
  ];
  const start = Math.max(0, props.selectedIndex - (props.visibleRows ?? 6) + 1);
  return (
    <Box flexDirection="column" borderStyle="round" borderColor={props.theme.border} paddingX={1} marginTop={1}>
      <Text color={props.theme.accent} wrap="truncate">Sessions</Text>
      {options.slice(start, start + (props.visibleRows ?? 6)).map((option, index) => (
        <PickerRow
          key={option.key}
          selected={props.selectedIndex === start + index}
          label={option.label}
          theme={props.theme}
        />
      ))}
      <Text color={props.theme.muted} wrap="truncate">↑/↓ select · Enter choose · Esc close</Text>
    </Box>
  );
}

function PickerRow(props: Readonly<{ selected: boolean; label: string; theme: BrainTheme }>) {
  return <Text color={props.selected ? props.theme.accent : props.theme.text} wrap="truncate">{props.selected ? '› ' : '  '}{props.label}</Text>;
}
