import * as React from 'react';
import * as path from 'path';
import { Box, Text, useTerminalSize } from '../../compat/index.js';
import { WelcomeFrame } from './WelcomeFrame.js';
import { StatusBarView } from './StatusBar.js';
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
import {
  resumeChoices,
  resumeListDecision,
  type ResumeVM,
} from '../overlays/resumePickerLogic.js';
import { ResumePickerView } from '../overlays/ResumePicker.js';
import { dialogDecision } from '../overlays/permissionDialogLogic.js';
import { PermissionDialogView } from '../overlays/PermissionDialog.js';
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
  const workspaceLabel = path.basename(process.cwd()).slice(0, 24);
  const [themeOpen, setThemeOpen] = React.useState(false);
  const [themeSelected, setThemeSelected] = React.useState(0);
  const [themeOriginal, setThemeOriginal] = React.useState<ThemeSetting>('auto');
  const [resumeOpen, setResumeOpen] = React.useState(false);
  const [resumeItems, setResumeItems] = React.useState<ResumeVM[]>([]);
  const [resumeSelected, setResumeSelected] = React.useState(0);
  const permission = snapshot.permission;
  const [permSelected, setPermSelected] = React.useState(0);
  React.useEffect(() => {
    setPermSelected(0);
  }, [permission?.callId]);

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

  // Resume picker overlay: same list grammar as /theme, but committing
  // hands the chosen session to the controller for adoption + replay.
  useBoundInput({
    contexts: ['overlay'],
    isActive: resumeOpen,
    onAction: (action) => {
      const d = resumeListDecision(action, resumeSelected, resumeItems.length);
      if (d.type === 'move') {
        setResumeSelected(d.index);
      } else if (d.type === 'commit') {
        setResumeOpen(false);
        const chosen = resumeItems[d.index];
        if (chosen) void controller.resumeSession(chosen.id);
      } else if (d.type === 'cancel') {
        setResumeOpen(false);
      }
    },
  });

  // Permission dialog: dismissal never grants — esc and n both deny.
  useBoundInput({
    contexts: ['dialog'],
    isActive: permission !== undefined,
    onAction: (action) => {
      if (!permission) return;
      const d = dialogDecision(action, permSelected);
      if (d.type === 'move') {
        setPermSelected(d.index);
      } else if (d.type === 'allow') {
        controller.resolvePermission(permission.callId, true);
      } else if (d.type === 'deny') {
        controller.resolvePermission(permission.callId, false);
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
    } else if (chosen.name === 'resume') {
      if (snapshot.busy) {
        controller.notice('Busy — wait for the current turn to finish.');
        return;
      }
      void controller.listSessions().then((all) => {
        const items = resumeChoices(all, Date.now());
        if (items.length === 0) {
          controller.notice('No previous sessions found.');
          return;
        }
        setResumeItems(items);
        setResumeSelected(0);
        setResumeOpen(true);
      });
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
      {resumeOpen ? (
        <Box marginTop={1}>
          <ResumePickerView items={resumeItems} selectedIndex={resumeSelected} tokens={tokens} />
        </Box>
      ) : null}
      {permission ? (
        <Box marginTop={1}>
          <PermissionDialogView req={permission} selected={permSelected} tokens={tokens} />
        </Box>
      ) : null}
      <Box marginTop={1}>
        <PromptInput
          disabled={false}
          busy={snapshot.busy}
          paused={themeOpen || resumeOpen || permission !== undefined}
          onSubmit={handleSubmit}
          onAbort={() => controller.abort()}
        />
      </Box>
      <StatusBarView
        model={model}
        workspace={workspaceLabel}
        theme={themeSetting}
        expandTools={expandTools}
        tokens={tokens}
      />
    </Box>
  );
}
