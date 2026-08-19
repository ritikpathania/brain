import * as React from 'react';
import { useExitOnCtrlCDWithKeybindings } from '../../vendor/claude/hooks/useExitOnCtrlCDWithKeybindings.js';
import { useTerminalSize } from '../../vendor/claude/hooks/useTerminalSize.js';
import { Box, Text, usePreviewTheme, useTheme, useThemeSetting } from '../../vendor/claude/ink.js';
import { useRegisterKeybindingContext } from '../../vendor/claude/keybindings/KeybindingContext.js';
import { useKeybinding } from '../../vendor/claude/keybindings/useKeybinding.js';
import { useShortcutDisplay } from '../../vendor/claude/keybindings/useShortcutDisplay.js';
import { useAppState, useSetAppState } from '../../vendor/claude/state/AppState.js';
import { gracefulShutdown } from '../../vendor/claude/utils/gracefulShutdown.js';
import { updateSettingsForSource } from '../../vendor/claude/utils/settings/settings.js';
import type { ThemeSetting } from '../../vendor/claude/utils/theme.js';
import { Select } from '../../vendor/claude/components/CustomSelect/index.js';
import { Byline } from '../../vendor/claude/components/design-system/Byline.js';
import { KeyboardShortcutHint } from '../../vendor/claude/components/design-system/KeyboardShortcutHint.js';
import { getColorModuleUnavailableReason, getSyntaxTheme } from '../../vendor/claude/components/StructuredDiff/colorDiff.js';
import { StructuredDiff } from '../../vendor/claude/components/StructuredDiff.js';

export type ThemePickerProps = {
  onThemeSelect: (setting: ThemeSetting) => void;
  showIntroText?: boolean;
  helpText?: string;
  showHelpTextBelow?: boolean;
  hideEscToCancel?: boolean;
  skipExitHandling?: boolean;
  onCancel?: () => void;
  onOpenCustomTheme?: (theme?: any) => void;
};

const SAMPLE_PATCH = {
  oldStart: 1,
  newStart: 1,
  oldLines: 3,
  newLines: 3,
  lines: [
    ' function greet() {',
    '-  console.log("Hello, World!");',
    '+  console.log("Hello, Claude!");',
    ' }',
  ],
};

export function ThemePicker({
  onThemeSelect,
  showIntroText = false,
  helpText = '',
  showHelpTextBelow = false,
  hideEscToCancel = false,
  skipExitHandling = false,
  onCancel: onCancelProp,
}: ThemePickerProps) {
  const [theme] = useTheme();
  const themeSetting = useThemeSetting();
  const { columns } = useTerminalSize();

  const colorModuleUnavailableReason = getColorModuleUnavailableReason();
  const syntaxTheme = colorModuleUnavailableReason === null ? getSyntaxTheme(theme) : null;
  const { setPreviewTheme, savePreview, cancelPreview } = usePreviewTheme();
  const syntaxHighlightingDisabled = useAppState((s: any) => s.settings?.syntaxHighlightingDisabled) ?? false;
  const setAppState = useSetAppState();

  useRegisterKeybindingContext('ThemePicker');
  const syntaxToggleShortcut = useShortcutDisplay('theme:toggleSyntaxHighlighting', 'ThemePicker', 'ctrl+t');

  const toggleSyntax = () => {
    if (colorModuleUnavailableReason === null) {
      const newValue = !syntaxHighlightingDisabled;
      updateSettingsForSource('userSettings', {
        syntaxHighlightingDisabled: newValue,
      });
      setAppState((prev: any) => ({
        ...prev,
        settings: {
          ...prev.settings,
          syntaxHighlightingDisabled: newValue,
        },
      }));
    }
  };

  useKeybinding('theme:toggleSyntaxHighlighting', toggleSyntax, {
    context: 'ThemePicker',
  });

  const exitState = useExitOnCtrlCDWithKeybindings(skipExitHandling ? () => {} : undefined);

  const themeOptions = [
    { label: 'Auto (match terminal)', value: 'auto' as const },
    { label: 'Dark mode', value: 'dark' as const },
    { label: 'Light mode', value: 'light' as const },
    { label: 'Dark mode (colorblind-friendly)', value: 'dark-daltonized' as const },
    { label: 'Light mode (colorblind-friendly)', value: 'light-daltonized' as const },
    { label: 'Dark mode (ANSI colors only)', value: 'dark-ansi' as const },
    { label: 'Light mode (ANSI colors only)', value: 'light-ansi' as const },
    { label: 'New custom theme…', value: 'custom-create' as const },
  ];

  const header = showIntroText ? (
    <Text>Let's get started.</Text>
  ) : (
    <Text bold color="permission">
      Theme
    </Text>
  );

  const title = <Text bold>Choose the text style that looks best with your terminal</Text>;
  const subtitle = helpText && !showHelpTextBelow ? <Text dimColor>{helpText}</Text> : null;
  const titleBox = (
    <Box flexDirection="column">
      {title}
      {subtitle}
    </Box>
  );

  const handleFocus = (setting: any) => {
    if (setting !== 'custom-create') {
      setPreviewTheme(setting as ThemeSetting);
    }
  };

  const handleChange = (setting: any) => {
    savePreview();
    onThemeSelect(setting as ThemeSetting);
  };

  const handleCancel = skipExitHandling
    ? () => {
        cancelPreview();
        onCancelProp?.();
      }
    : async () => {
        cancelPreview();
        await gracefulShutdown(0);
      };

  const selectElement = (
    <Select
      options={themeOptions}
      onFocus={handleFocus}
      onChange={handleChange}
      onCancel={handleCancel}
      visibleOptionCount={themeOptions.length}
      defaultValue={themeSetting}
      defaultFocusValue={themeSetting}
    />
  );

  const topSection = (
    <Box flexDirection="column" gap={1}>
      {header}
      {titleBox}
      {selectElement}
    </Box>
  );

  const diffWidth = showIntroText ? columns : columns - 6;

  const diffBox = (
    <Box
      flexDirection="column"
      borderTop
      borderBottom
      borderLeft={false}
      borderRight={false}
      borderStyle="dashed"
      borderColor="subtle"
    >
      <StructuredDiff patch={SAMPLE_PATCH} dim={false} filePath="demo.js" firstLine={null} width={diffWidth} />
    </Box>
  );

  const statusText =
    colorModuleUnavailableReason === 'env'
      ? `Syntax highlighting disabled (via CLAUDE_CODE_SYNTAX_HIGHLIGHT=${process.env.CLAUDE_CODE_SYNTAX_HIGHLIGHT})`
      : syntaxHighlightingDisabled
      ? `Syntax highlighting disabled (${syntaxToggleShortcut} to enable)`
      : syntaxTheme
      ? `Syntax theme: ${syntaxTheme.theme}${syntaxTheme.source ? ` (from ${syntaxTheme.source})` : ''} (${syntaxToggleShortcut} to disable)`
      : `Syntax highlighting enabled (${syntaxToggleShortcut} to disable)`;

  const bottomSection = (
    <Box flexDirection="column" width="100%">
      {diffBox}
      <Text dimColor> {statusText}</Text>
    </Box>
  );

  const content = (
    <Box flexDirection="column" gap={1}>
      {topSection}
      {bottomSection}
    </Box>
  );

  if (!showIntroText) {
    const bottomHelp =
      showHelpTextBelow && helpText ? (
        <Box marginLeft={3}>
          <Text dimColor>{helpText}</Text>
        </Box>
      ) : null;

    const shortcuts = !hideEscToCancel ? (
      <Box>
        <Text dimColor italic>
          {exitState.pending ? (
            <>Press {exitState.keyName} again to exit</>
          ) : (
            <Byline>
              <KeyboardShortcutHint shortcut="Enter" action="select" />
              <KeyboardShortcutHint shortcut="Esc" action="cancel" />
            </Byline>
          )}
        </Text>
      </Box>
    ) : null;

    return (
      <>
        <Box flexDirection="column">{content}</Box>
        <Box marginTop={1}>
          {bottomHelp}
          {shortcuts}
        </Box>
      </>
    );
  }

  return content;
}
