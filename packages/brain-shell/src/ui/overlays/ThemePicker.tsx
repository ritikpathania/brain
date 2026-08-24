/**
 * /theme overlay: five settings (auto + four palettes). Navigation calls
 * setSetting live — that IS the preview; esc rolls back, enter persists
 * via the theme store. Rounded border per TUI rules.
 */
import * as React from 'react';
import { Box, Text } from '../../compat/index.js';
import type { BrainTokens } from '../../state/palettes.js';
import type { ThemeSetting } from '../../contracts/theme.js';

export interface ThemeChoice {
  setting: ThemeSetting;
  label: string;
}

export const THEME_CHOICES: readonly ThemeChoice[] = [
  { setting: 'auto', label: 'Auto (detect terminal)' },
  { setting: 'dark', label: 'Dark' },
  { setting: 'light', label: 'Light' },
  { setting: 'dark-daltonized', label: 'Dark (daltonized)' },
  { setting: 'light-daltonized', label: 'Light (daltonized)' },
];

export function ThemePickerView(props: {
  choices: readonly ThemeChoice[];
  selectedIndex: number;
  current: ThemeSetting;
  tokens: BrainTokens;
}): React.ReactElement {
  const sel = Math.min(props.selectedIndex, Math.max(0, props.choices.length - 1));
  return (
    <Box flexDirection="column" borderStyle="round" borderColor={props.tokens.promptBorder} paddingX={1}>
      <Text bold>Theme</Text>
      {props.choices.map((c, i) => (
        <Text key={c.setting} inverse={i === sel}>
          {i === sel ? '❯ ' : '  '}
          {c.label}
          {c.setting === props.current ? `  ✓ ${c.setting}` : ''}
        </Text>
      ))}
      <Text dimColor>↑↓ navigate (live preview) · enter apply · esc cancel</Text>
    </Box>
  );
}
