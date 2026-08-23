import * as React from 'react';
import { Text, useTheme } from '../../compat/index.js';
import type { BrainTokens } from '../../state/palettes.js';
import type { LiveStreamView } from '../../contracts/streaming.js';

/** Palindrome bounce, 120 ms cadence (reference Spinner contract). */
export const spinnerFrames: readonly string[] = ['✢', '✳', '∗', '✻', '∗', '✳'];
const FRAME_MS = 120;

export function spinnerFrameAt(elapsedMs: number): string {
  const idx = Math.floor(Math.max(0, elapsedMs) / FRAME_MS) % spinnerFrames.length;
  return spinnerFrames[idx]!;
}

export function spinnerLabel(live: LiveStreamView): string {
  switch (live.phase) {
    case 'thinking':
      return 'Thinking…';
    case 'responding':
      return 'Composing…';
    case 'tool':
      return `${live.activeToolName ?? 'Working'}…`;
    case 'error':
      return 'Failed';
    default:
      return '';
  }
}

/** Pure view: frame + label at a given elapsed time. */
export function SpinnerView(props: {
  elapsedMs: number;
  label: string;
  tokens: BrainTokens;
}): React.ReactElement {
  return (
    <Text>
      <Text color={props.tokens.brandShimmer}>{spinnerFrameAt(props.elapsedMs)}</Text>
      {props.label.length > 0 ? <Text dimColor>{` ${props.label}`}</Text> : null}
    </Text>
  );
}

export function Spinner(props: { label: string }): React.ReactElement {
  const { tokens } = useTheme();
  const [start] = React.useState(() => Date.now());
  const [, forceTick] = React.useReducer((n: number) => n + 1, 0);
  React.useEffect(() => {
    const t = setInterval(forceTick, FRAME_MS);
    return () => clearInterval(t);
  }, []);
  void start;
  return <SpinnerView elapsedMs={Date.now() - start} label={props.label} tokens={tokens} />;
}
