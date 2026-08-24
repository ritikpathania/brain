import * as React from 'react';
import { Box, Text, useInput, useTheme, useTerminalSize } from '../../compat/index.js';
import type { Key } from '../../compat/index.js';
import type { BrainTokens } from '../../state/palettes.js';
import { parseCommandQuery, fuzzyMatchCommands } from '../../commands/matcher.js';
import { paletteKeyDecision } from './paletteLogic.js';
import { PaletteView } from './PaletteView.js';
import type { PaletteItemVM } from './PaletteView.js';
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
    tab: (key as { tab?: boolean }).tab,
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
  /** While true (an overlay/dialog is up upstream) the editor ignores all input. */
  paused?: boolean;
  onSubmit: (value: string, mode: 'prompt' | 'bash') => void;
  onAbort?: () => void;
}): React.ReactElement {
  const { tokens } = useTheme();
  const { columns } = useTerminalSize();
  const [state, setState] = React.useState<ComposerState>(() => createComposerState(loadHistory()));
  const [selected, setSelected] = React.useState(0);
  const [suppressed, setSuppressed] = React.useState(false);

  // Palette is open iff the whole buffer is a bare slash query. Esc sets
  // `suppressed` (esc means abort once the menu is dismissed); clearing the
  // buffer re-arms it.
  const query = parseCommandQuery(state.value);
  const matches =
    query !== null && !suppressed && !(props.busy ?? false)
      ? fuzzyMatchCommands(query)
      : [];
  const paletteItems: PaletteItemVM[] = matches.map((m) => ({
    name: m.command.name,
    description: m.command.description,
  }));
  const paletteOpen = paletteItems.length > 0;
  React.useEffect(() => {
    setSelected(0);
  }, [query]);
  React.useEffect(() => {
    if (state.value.length === 0) setSuppressed(false);
  }, [state.value]);

  useInput((input, key) => {
    if (props.disabled) return;
    const info = asKeyInfo(key);
    const cmd = translateKey(input, info);
    const decision = paletteKeyDecision({
      open: paletteOpen,
      cmdType: cmd.type,
      tab: info.tab ?? false,
      selected: Math.min(selected, Math.max(0, matches.length - 1)),
      count: matches.length,
    });
    if (decision.kind === 'move') {
      setSelected(decision.next);
      return;
    }
    if (decision.kind === 'complete') {
      const chosen = matches[decision.index]!.command;
      setState((s) => reduceComposer(s, { type: 'replace_all', value: `/${chosen.name} ` }));
      return;
    }
    if (decision.kind === 'close') {
      setSuppressed(true);
      return;
    }
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
      props.onSubmit(bare, wasBash ? 'bash' : 'prompt');
      return;
    }
    if (cmd.type !== 'noop') {
      setState((s) => reduceComposer(s, cmd));
    }
  }, { isActive: !(props.paused ?? false) });

  return (
    <Box flexDirection="column">
      {paletteOpen ? (
        <PaletteView
          items={paletteItems}
          selectedIndex={Math.min(selected, paletteItems.length - 1)}
          maxColumns={columns}
          tokens={tokens}
        />
      ) : null}
      <PromptInputView value={state.value} cursor={state.cursor} busy={props.busy ?? false} tokens={tokens} />
    </Box>
  );
}
