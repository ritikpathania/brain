/** Footer status line: workspace/model/theme context + keybind hints. */
import * as React from 'react';
import { Text } from '../../compat/index.js';
import type { BrainTokens } from '../../state/palettes.js';

export function StatusBarView(props: {
  model: string;
  workspace: string;
  theme: string;
  expandTools: boolean;
  tokens: BrainTokens;
}): React.ReactElement {
  void props.tokens; // reserved: segments gain token colors in later increments
  return (
    <Text dimColor>
      {props.workspace} · model {props.model} · theme {props.theme} · ! bash · / commands · ↑↓
      history · esc stop · ctrl+o {props.expandTools ? 'collapse' : 'expand'} tools · ctrl+c exit
    </Text>
  );
}
