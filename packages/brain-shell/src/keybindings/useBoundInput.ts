import { useInput } from '../compat/index.js';
import type { Key } from '../compat/index.js';
import { DEFAULT_BINDINGS, resolveAction, strokeToKey } from './resolve.js';
import type { BindingRule, KeybindingContextName } from './resolve.js';

/**
 * React seam over the keybinding resolver: fires onAction for bound strokes
 * in the given contexts, ignores everything else. Handlers stay in the
 * component; the table stays data.
 */
export function useBoundInput(opts: {
  contexts: KeybindingContextName[];
  bindings?: readonly BindingRule[];
  isActive?: boolean;
  onAction: (action: string, input: string, key: Key) => void;
}): void {
  const { contexts, bindings = DEFAULT_BINDINGS, isActive = true, onAction } = opts;
  useInput(
    (input, key) => {
      const keyId = strokeToKey(input, key);
      const action = resolveAction(bindings, contexts, keyId);
      if (action !== null) onAction(action, input, key);
    },
    { isActive },
  );
}
