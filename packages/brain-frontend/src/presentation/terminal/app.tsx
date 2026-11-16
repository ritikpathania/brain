import { Box, useApp, useInput, useWindowSize } from 'ink';
import { useEffect, useState } from 'react';

import { PromptComposer } from '../components/promptComposer.js';
import { SessionPicker } from '../components/sessionPicker.js';
import { StatusLine } from '../components/statusLine.js';
import { Transcript } from '../components/transcript.js';
import { PermissionPrompt } from '../components/permission.js';
import { ContextView } from '../components/context.js';
import { toTerminalModel } from '../models.js';
import type { BrainTheme } from '../theme.js';
import { routeTerminalKey, type TerminalKey } from '../input/keyRouter.js';
import { composerViewport } from '../input/composerViewport.js';
import type { FrontendInteractionSnapshot, BrainFrontendController } from '../../state/interactions.js';

type TerminalController = Pick<BrainFrontendController,
  | 'appendDraft'
  | 'deleteDraftCharacter'
  | 'insertNewline'
  | 'submit'
  | 'cancel'
  | 'openSessionPicker'
  | 'closeSessionPicker'
  | 'moveSessionSelection'
  | 'selectHighlightedSession'
  | 'scrollUp'
  | 'scrollToBottom'
  | 'moveComposerCursor'
  | 'resolvePermission'
  | 'closeContext'
  | 'toggleToolDetail'
>;

export function BrainTerminalApp(props: Readonly<{ controller: BrainFrontendController; theme: BrainTheme }>) {
  const [state, setState] = useState(props.controller.snapshot());
  const { exit } = useApp();
  const size = useWindowSize();
  useEffect(() => {
    const unsubscribe = props.controller.subscribe(setState);
    setState(props.controller.snapshot());
    const monitor = setInterval(() => { void props.controller.refreshConnection(); }, 2_000);
    return () => { clearInterval(monitor); unsubscribe(); };
  }, [props.controller]);
  useInput((input, key) => {
    if (key.eventType === 'release') return;
    if (key.ctrl && input.toLowerCase() === 'c' && !state.generating && state.focus === 'composer') {
      exit();
      return;
    }
    void dispatchTerminalKey(toTerminalKey(input, key), state, props.controller);
  });
  return <BrainTerminalScreen model={state} theme={props.theme} rows={size.rows} columns={size.columns} />;
}

export function BrainTerminalScreen(props: Readonly<{ model: FrontendInteractionSnapshot; theme: BrainTheme; rows?: number; columns?: number }>) {
  const model = toTerminalModel(props.model);
  const rows = Math.max(8, props.rows ?? 24);
  const interactionRows = model.permission !== undefined ? 4 : model.focus === 'context' ? 3 : 0;
  const showComposer = (!model.sessionPickerOpen && interactionRows === 0) || rows >= 14;
  const showScrollHint = !model.followTranscript && rows >= 12 && showComposer;
  const scrollHintRows = showScrollHint ? 2 : 0;
  const composerLimit = Math.max(1, Math.min(4, rows - 6 - scrollHintRows - interactionRows - (model.sessionPickerOpen ? 6 : 0)));
  const composer = composerViewport(model.draft, model.draftCursor, Math.max(2, (props.columns ?? 80) - 4), composerLimit);
  const controlsRows = showComposer ? composer.length + 5 : 2;
  const pickerRows = Math.max(1, Math.min(6, model.sessions.length + 1, rows - controlsRows - scrollHintRows - 5));
  const transcriptRows = Math.max(0, rows - controlsRows - (model.sessionPickerOpen ? pickerRows + 5 : 0) - scrollHintRows - interactionRows);
  return (
    <Box flexDirection="column" paddingX={1} width={props.columns}>
      <Box height={transcriptRows} overflowY="hidden" justifyContent="flex-end" flexDirection="column">
        <Transcript entries={model.entries} theme={props.theme} />
      </Box>
      {showScrollHint && <StatusLine connection={model.connection} generating={model.generating} cancelling={model.cancelling} sessionTitle="Viewing earlier messages · PageDown to follow" theme={props.theme} />}
      {model.sessionPickerOpen && <SessionPicker sessions={model.sessions} selectedIndex={model.selectedSessionIndex} visibleRows={pickerRows} theme={props.theme} />}
      {model.permission !== undefined && <PermissionPrompt permission={model.permission} theme={props.theme} />}
      {model.focus === 'context' && <ContextView summary={model.contextSummary} theme={props.theme} />}
      {showComposer && <PromptComposer lines={composer} generating={model.generating} cancelling={model.cancelling} theme={props.theme} />}
      <StatusLine
        connection={model.connection}
        sessionTitle={model.sessionTitle}
        generating={model.generating}
        cancelling={model.cancelling}
        theme={props.theme}
      />
    </Box>
  );
}

export async function dispatchTerminalKey(
  key: TerminalKey,
  state: FrontendInteractionSnapshot,
  controller: TerminalController,
): Promise<void> {
  const intent = routeTerminalKey(key, { focus: state.focus, generating: state.generating });
  switch (intent) {
    case 'permission-allow':
      await controller.resolvePermission('allow');
      return;
    case 'permission-deny':
      await controller.resolvePermission('deny');
      return;
    case 'dismiss-context':
      controller.closeContext();
      return;
    case 'toggle-tool-detail':
      controller.toggleToolDetail();
      return;
    case 'append':
      controller.appendDraft(key.input);
      return;
    case 'delete-backward':
      controller.deleteDraftCharacter();
      return;
    case 'newline':
      controller.insertNewline();
      return;
    case 'submit':
      await controller.submit();
      return;
    case 'cancel':
      await controller.cancel();
      return;
    case 'open-session-picker':
      await controller.openSessionPicker();
      return;
    case 'dismiss-overlay':
      controller.closeSessionPicker();
      return;
    case 'picker-next':
      controller.moveSessionSelection(1);
      return;
    case 'picker-previous':
      controller.moveSessionSelection(-1);
      return;
    case 'picker-select':
      await controller.selectHighlightedSession();
      return;
    case 'scroll-up':
      controller.scrollUp();
      return;
    case 'scroll-bottom':
      controller.scrollToBottom();
      return;
    case 'cursor-left':
    case 'cursor-right':
    case 'cursor-up':
    case 'cursor-down':
    case 'cursor-home':
    case 'cursor-end':
      controller.moveComposerCursor(intent.slice(7) as 'left' | 'right' | 'up' | 'down' | 'home' | 'end');
      return;
    case 'ignore':
      return;
  }
}

function toTerminalKey(input: string, key: Parameters<typeof useInput>[0] extends (input: string, key: infer K) => void ? K : never): TerminalKey {
  return {
    input,
    return: key.return,
    escape: key.escape,
    ctrl: key.ctrl,
    shift: key.shift,
    pageUp: key.pageUp,
    pageDown: key.pageDown,
    upArrow: key.upArrow,
    downArrow: key.downArrow,
    backspace: key.backspace,
    leftArrow: key.leftArrow,
    rightArrow: key.rightArrow,
    home: key.home,
    end: key.end,
  };
}
