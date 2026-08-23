import * as React from 'react';
import { Text } from '../../compat/ink.js';

export type PromptInputMode = 'prompt' | 'bash';

/** Inline mode badge shown left of the composer. Bash mode gets the `!` affordance. */
export function PromptModeIndicator({ mode }: { mode: PromptInputMode }): React.ReactElement {
  return mode === 'bash'
    ? <Text bold color="yellow">! bash</Text>
    : <></>;
}
