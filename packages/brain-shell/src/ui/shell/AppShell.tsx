import * as React from 'react';
import { Box, Text, useTerminalSize } from '../../compat/index.js';
import { WelcomeFrame } from './WelcomeFrame.js';
import { Spinner, spinnerLabel } from './Spinner.js';
import { MessageRow } from '../transcript/MessageRow.js';
import { PromptInput } from '../composer/PromptInput.js';
import { SessionController } from '../../state/sessionController.js';
import { UdsBrainBackendClient } from '../../client/UdsBrainBackendClient.js';
import { useMainLoopModel } from '../../contracts/model.js';
import { useShellSnapshot } from './useShellSnapshot.js';
import { useBoundInput } from '../../keybindings/useBoundInput.js';
import { COMMANDS } from '../../commands/matcher.js';
import { useTheme } from '../../compat/index.js';
import { overlayListDecision } from '../overlays/overlayLogic.js';
import { THEME_CHOICES, ThemePickerView } from '../overlays/ThemePicker.js';
import { writeThemeSetting } from '../../state/themeStore.js';
import type { ThemeSetting } from '../../contracts/theme.js';

/**
 * Top-level live shell: frozen transcript + streaming block + composer.
 * Static/live split via React.memo rows (not ink <Static>) so ctrl+o can
 * re-render past tool cards.
 */
export function AppShell(): React.ReactElement {
  const { columns } = useTerminalSize();
  const model = useMainLoopModel(); // hoisted — hooks never inside JSX
  // One controller per mount; UDS path resolves from BRAIN_SOCKET_PATH.
  const controller = React.useMemo(
    () => new SessionController(new UdsBrainBackendClient()),
    [],
  );
  const snapshot = useShellSnapshot(controller);
  const [expandTools, setExpandTools] = React.useState(false);
  const { setting: themeSetting, tokens, setSetting } = useTheme();
  const [themeOpen, setThemeOpen] = React.useState(false);
  const [themeSelected, setThemeSelected] = React.useState(0);
  const [themeOriginal, setThemeOriginal] = React.useState<ThemeSetting>('auto');

  useBoundInput({
    contexts: ['global'],
    onAction: (action) => {
      if (action === 'shell:exit') process.exit(0);
      if (action === 'shell:toggleTools') setExpandTools((v) => !v);
    },
  });

  // Theme picker overlay: navigating previews live (setSetting), esc rolls
  // back to the setting captured at open, enter persists via the store.
  useBoundInput({
    contexts: ['overlay'],
    isActive: themeOpen,
    onAction: (action) => {
      const d = overlayListDecision(action, themeSelected, THEME_CHOICES.length);
      if (d.type === 'move') {
        setThemeSelected(d.index);
        setSetting(THEME_CHOICES[d.index]!.setting); // live preview
      } else if (d.type === 'commit') {
        setThemeOpen(false);
        try {
          writeThemeSetting(THEME_CHOICES[d.index]!.setting);
        } catch {
          controller.notice('Could not save the theme setting.');
        }
      } else if (d.type === 'cancel') {
        setSetting(themeOriginal); // rollback preview
        setThemeOpen(false);
      }
    },
  });

  const helpText = (): string =>
    ['Slash commands:', ...COMMANDS.map((c) => `/${c.name} — ${c.description}`)].join('\n');

  const runCommand = (rawValue: string): void => {
    const token = rawValue.trim().slice(1).toLowerCase(); // strip '/', tolerate trailing space
    if (token.length === 0) return;
    const exact = COMMANDS.find(
      (c) => c.name === token || (c.aliases ?? []).includes(token),
    );
    let chosen = exact;
    if (chosen === undefined) {
      const prefixHits = COMMANDS.filter((c) => c.name.startsWith(token));
      if (prefixHits.length === 1) chosen = prefixHits[0];
      else if (prefixHits.length > 1) {
        controller.notice(`Ambiguous command: /${token}`);
        return;
      } else {
        controller.notice(`Unknown command: /${token}`);
        return;
      }
    }
    if (chosen.name === 'help') controller.notice(helpText());
    else if (chosen.name === 'clear') controller.clear();
    else if (chosen.name === 'theme') {
      setThemeOriginal(themeSetting);
      setThemeSelected(Math.max(0, THEME_CHOICES.findIndex((c) => c.setting === themeSetting)));
      setThemeOpen(true);
    } else if (chosen.name === 'quit') process.exit(0);
  };

  const handleSubmit = (text: string): void => {
    if (text.trimStart().startsWith('/')) runCommand(text);
    else void controller.submit(text);
  };

  const lastThinking =
    snapshot.live.thinkingText.length > 0
      ? snapshot.live.thinkingText.trimEnd().split('\n').slice(-1)[0]!
      : '';

  return (
    <Box flexDirection="column" width={columns}>
      {snapshot.rows.length === 0 && !snapshot.busy ? <WelcomeFrame /> : null}
      {snapshot.rows.map((row) => (
        <MessageRow key={row.id} row={row} expanded={expandTools} />
      ))}
      {snapshot.busy ? (
        <Box marginTop={1} flexDirection="column">
          <Spinner label={spinnerLabel(snapshot.live)} />
          {lastThinking.length > 0 ? (
            <Text dimColor italic>
              {lastThinking}
            </Text>
          ) : null}
          {snapshot.live.responseText.length > 0 ? (
            <Text>{snapshot.live.responseText}</Text>
          ) : null}
        </Box>
      ) : null}
      {snapshot.connectionError !== undefined ? (
        <Text color="red">⚠ {snapshot.connectionError}</Text>
      ) : null}
      {themeOpen ? (
        <Box marginTop={1}>
          <ThemePickerView
            choices={THEME_CHOICES}
            selectedIndex={themeSelected}
            current={themeSetting}
            tokens={tokens}
          />
        </Box>
      ) : null}
      <Box marginTop={1}>
        <PromptInput
          disabled={false}
          busy={snapshot.busy}
          paused={themeOpen}
          onSubmit={handleSubmit}
          onAbort={() => controller.abort()}
        />
      </Box>
      <Text dimColor>
        model: {model} · ! bash · / commands · ↑↓ history · esc stop · ctrl+o{' '}
        {expandTools ? 'collapse' : 'expand'} tools · ctrl+c exit
      </Text>
    </Box>
  );
}
