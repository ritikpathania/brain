import * as React from 'react';
import * as path from 'path';
import { Box, Text, useTerminalSize } from '../../compat/index.js';
import { WelcomeFrame } from './WelcomeFrame.js';
import { StatusBarView } from './StatusBar.js';
import { connectionStatusText } from './connectionStatusLogic.js';
import { Spinner, spinnerLabel } from './Spinner.js';
import { MessageRow } from '../transcript/MessageRow.js';
import { PromptInput } from '../composer/PromptInput.js';
import { SessionController } from '../../state/sessionController.js';
import { UdsBrainBackendClient } from '../../client/UdsBrainBackendClient.js';
import { useMainLoopModel } from '../../contracts/model.js';
import { useShellSnapshot } from './useShellSnapshot.js';
import { useBoundInput } from '../../keybindings/useBoundInput.js';
import { getCommand, getCommands } from '../../commands/registry.js';
import '../../commands/builtin.js';
import { useTheme } from '../../compat/index.js';
import { overlayListDecision } from '../overlays/overlayLogic.js';
import { THEME_CHOICES, ThemePickerView } from '../overlays/ThemePicker.js';
import {
  applyQueryEdit,
  resumeChoices,
  resumeListDecision,
} from '../overlays/resumePickerLogic.js';
import type { BrainSessionSummary } from '../../client/BrainBackendClient.js';
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
  // Inc 15: unmount must cancel the reconnect loop's timers.
  React.useEffect(() => () => controller.dispose(), [controller]);
  const snapshot = useShellSnapshot(controller);
  const [expandTools, setExpandTools] = React.useState(false);
  const { setting: themeSetting, tokens, setSetting } = useTheme();
  const workspaceLabel = path.basename(process.cwd()).slice(0, 24);
  const [themeOpen, setThemeOpen] = React.useState(false);
  const [themeSelected, setThemeSelected] = React.useState(0);
  const [themeOriginal, setThemeOriginal] = React.useState<ThemeSetting>('auto');
  const [resumeOpen, setResumeOpen] = React.useState(false);
  const [resumeSummaries, setResumeSummaries] = React.useState<BrainSessionSummary[]>([]);
  const [resumeSelected, setResumeSelected] = React.useState(0);
  const [resumeQuery, setResumeQuery] = React.useState('');
  // Command-surface overlays (Inc 21): /doctor and /memory mount points.
  const [doctorOpen, setDoctorOpen] = React.useState(false);
  const [memoryOpen, setMemoryOpen] = React.useState(false);
  const resumeItems = React.useMemo(
    () => resumeChoices(resumeSummaries, Date.now(), resumeQuery),
    [resumeSummaries, resumeQuery],
  );
  const permission = snapshot.permission;
  const [permSelected, setPermSelected] = React.useState(0);
  React.useEffect(() => {
    setPermSelected(0);
  }, [permission?.callId]);
  React.useEffect(() => {
    setResumeSelected((i) => Math.min(i, Math.max(0, resumeItems.length - 1)));
  }, [resumeItems.length]);

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
    onAction: (action, input) => {
      const d = resumeListDecision(action, resumeSelected, resumeItems.length);
      if (d.type === 'move') {
        setResumeSelected(d.index);
      } else if (d.type === 'commit') {
        setResumeOpen(false);
        const chosen = resumeItems[d.index];
        if (chosen) void controller.resumeSession(chosen.id);
      } else if (d.type === 'cancel') {
        setResumeOpen(false);
      } else if (action === 'overlay:insert') {
        setResumeQuery((q) => applyQueryEdit(q, action, input));
      } else if (action === 'overlay:backspace') {
        setResumeQuery((q) => applyQueryEdit(q, action, input));
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
      } else if (d.type === 'always') {
        controller.resolvePermissionAlways(permission.callId);
      } else if (d.type === 'deny') {
        controller.resolvePermission(permission.callId, false);
      }
    },
  });

  const runCommand = (rawValue: string): void => {
    const words = rawValue.trim().slice(1).split(/\s+/); // strip '/', split args
    const token = (words[0] ?? '').toLowerCase();
    const args = words.slice(1);
    if (token.length === 0) return;
    let chosen = getCommand(token);
    if (chosen === undefined) {
      const prefixHits = getCommands().filter((c) => c.name.startsWith(token));
      if (prefixHits.length === 1) chosen = prefixHits[0];
      else if (prefixHits.length > 1) {
        controller.notice(`Ambiguous command: /${token}`);
        return;
      } else {
        controller.notice(`Unknown command: /${token}`);
        return;
      }
    }
    // Commands return declarative results; the shell interprets them.
    const res = chosen.run({ args, sessionId: controller.activeSessionId });
    switch (res.type) {
      case 'text':
        controller.notice(res.value);
        break;
      case 'none':
        break;
      case 'action':
        if (res.action === 'quit') process.exit(0);
        else if (res.action === 'clear') controller.clear();
        else if (res.action === 'theme') {
          setThemeOriginal(themeSetting);
          setThemeSelected(Math.max(0, THEME_CHOICES.findIndex((c) => c.setting === themeSetting)));
          setThemeOpen(true);
        } else if (res.action === 'resume') {
          if (snapshot.busy) {
            controller.notice('Busy — wait for the current turn to finish.');
            return;
          }
          void controller.listSessions().then((all) => {
            if (resumeChoices(all, Date.now()).length === 0) {
              controller.notice('No previous sessions found.');
              return;
            }
            setResumeSummaries(all);
            setResumeQuery('');
            setResumeSelected(0);
            setResumeOpen(true);
          });
        }
        break;
      case 'overlay':
        if (res.overlay === 'doctor') setDoctorOpen(true);
        else setMemoryOpen(true);
        break;
    }
  };

  const handleSubmit = (text: string, mode: 'prompt' | 'bash' = 'prompt'): void => {
    if (mode === 'bash') void controller.runShellCommand(text);
    else if (text.trimStart().startsWith('/')) runCommand(text);
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
      {snapshot.connection.status !== 'connected' ? (
        <Text color="yellow">⚠ Connection lost — reconnecting…</Text>
      ) : snapshot.connectionError !== undefined ? (
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
          <ResumePickerView
            items={resumeItems}
            selectedIndex={resumeSelected}
            tokens={tokens}
            query={resumeQuery}
            currentSessionId={controller.activeSessionId}
          />
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
        connectionText={connectionStatusText(snapshot.connection)}
      />
    </Box>
  );
}
