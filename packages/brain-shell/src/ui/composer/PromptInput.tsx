import * as React from 'react';
import { Box, Text, useInput, useTheme } from '../../compat/index.js';
import type { Key } from '../../compat/index.js';
import type { BrainTokens } from '../../state/palettes.js';
import {
  createComposerState,
  reduceComposer,
  modeOf,
  expandedValue,
} from './composerState.js';
import type { ComposerState } from './composerState.js';
import { translateKey } from './translateKey.js';
import type { KeyInfo } from './translateKey.js';
import { loadHistory, appendHistory } from './historyStore.js';

function asKeyInfo(key: Key): KeyInfo {
  return {
    upArrow: key.upArrow,
    downArrow: key.downArrow,
    leftArrow: key.leftArrow,
    rightArrow: key.rightArrow,
    return: key.return,
    escape: key.escape,
    ctrl: key.ctrl,
    meta: key.meta,
    shift: key.shift,
    backspace: (key as { backspace?: boolean }).backspace,
    delete: key.delete,
  };
}

/**
 * Pure view: mode glyph + buffer with block cursor. Takes tokens explicitly
 * (repo convention) so tests assert output without mounting a reconciler.
 */
export function PromptInputView(props: {
  value: string;
  cursor: number;
  busy: boolean;
  tokens: BrainTokens;
}): React.ReactElement {
  const { value, cursor, busy, tokens } = props;
  const mode = modeOf(value);
  const glyph = mode === 'bash' ? '!' : '❯';
  const glyphColor = busy ? tokens.promptBorderInactive : tokens.promptBorder;
  const before = value.slice(0, cursor);
  const at = value.slice(cursor, cursor + 1);
  const after = value.slice(cursor + 1);
  return (
    <Box>
      <Text color={glyphColor}>{glyph} </Text>
      <Text>
        {before}
        <Text inverse>{at.length > 0 ? at : ' '}</Text>
        {after}
      </Text>
    </Box>
  );
}

export function PromptInput(props: {
  disabled?: boolean;
  busy?: boolean;
  onSubmit: (value: string) => void;
  onAbort?: () => void;
}): React.ReactElement {
  const { tokens } = useTheme();
  const [state, setState] = React.useState<ComposerState>(() => createComposerState(loadHistory()));

  useInput((input, key) => {
    if (props.disabled) return;
    const cmd = translateKey(input, asKeyInfo(key));
    if (cmd.type === 'exit') {
      process.exit(0);
      return;
    }
    if (cmd.type === 'abort') {
      props.onAbort?.();
      return;
    }
    if (cmd.type === 'submit') {
      // Reading `state` (not the updater form) is safe here: ink serializes
      // keystrokes through one handler, so `state` is fresh at each event.
      // props.onSubmit must stay outside the updater — side effects are
      // forbidden inside state functions.
      const value = expandedValue(state).trim();
      if (value.length === 0) return;
      const wasBash = modeOf(value) === 'bash';
      const bare = wasBash ? value.slice(1).trimStart() : value;
      const entry = { mode: wasBash ? ('bash' as const) : ('prompt' as const), value: bare };
      setState((s) => reduceComposer(s, { type: 'submit_done', entry }));
      appendHistory(entry);
      props.onSubmit(bare);
      return;
    }
    if (cmd.type !== 'noop') {
      setState((s) => reduceComposer(s, cmd));
    }
  });

  return <PromptInputView value={state.value} cursor={state.cursor} busy={props.busy ?? false} tokens={tokens} />;
}
