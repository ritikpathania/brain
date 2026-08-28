/** Footer status line: workspace/model/thinking/theme context + keybind hints. */
import * as React from 'react';
import { Text } from '../../compat/index.js';
import type { BrainTokens } from '../../state/palettes.js';

export function formatThinkingIndicator(
  supportsThinking: boolean | undefined,
  budget: number | null | undefined,
): string {
  if (supportsThinking === false) {
    return 'think:n/a';
  }
  if (budget === null || budget === undefined || budget === 0) {
    return 'think:off';
  }
  if (budget >= 1000) {
    const k = Math.round(budget / 1000);
    return `think:${k}k`;
  }
  return `think:${budget}`;
}

export function StatusBarView(props: {
  model: string;
  workspace: string;
  theme: string;
  expandTools: boolean;
  tokens: BrainTokens;
  thinkingBudget?: number | null;
  supportsThinking?: boolean;
  connectionText?: string | null;
}): React.ReactElement {
  void props.tokens; // reserved: segments gain token colors in later increments
  const thinkingLabel = formatThinkingIndicator(props.supportsThinking, props.thinkingBudget);
  return (
    <Text dimColor>
      {props.workspace} · model {props.model} · {thinkingLabel} · theme {props.theme} · ! bash · /
      commands · ↑↓ history · esc stop · ctrl+o {props.expandTools ? 'collapse' : 'expand'} tools
      {props.connectionText ? <Text color="yellow"> · {props.connectionText}</Text> : null} ·
      ctrl+c exit
    </Text>
  );
}
