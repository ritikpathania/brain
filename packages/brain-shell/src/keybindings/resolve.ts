/**
 * Context-scoped keybinding resolver. Binding table (keystroke → namespaced
 * action id) is data; handlers stay in components. Resolution walks the
 * caller's context list most-specific-first and consults 'global' last.
 */
import type { KeyInfo } from '../ui/composer/translateKey.js';

export type KeybindingContextName = 'global' | 'composer' | 'palette' | 'overlay' | 'dialog';

export interface BindingRule {
  action: string;
  context: KeybindingContextName;
  /** Canonical key id from strokeToKey, e.g. 'ctrl+c', 'return', 'tab'. */
  key: string;
}

/** The shell's default table. Later increments extend, never reorder. */
export const DEFAULT_BINDINGS: readonly BindingRule[] = [
  { action: 'shell:exit', context: 'global', key: 'ctrl+c' },
  { action: 'shell:toggleTools', context: 'global', key: 'ctrl+o' },
  { action: 'composer:submit', context: 'composer', key: 'return' },
  { action: 'composer:abort', context: 'composer', key: 'escape' },
  // Overlay lists (theme picker, resume picker): arrow-navigate, enter picks, esc closes.
  { action: 'overlay:up', context: 'overlay', key: 'up' },
  { action: 'overlay:down', context: 'overlay', key: 'down' },
  { action: 'overlay:commit', context: 'overlay', key: 'return' },
  { action: 'overlay:cancel', context: 'overlay', key: 'escape' },
  // Permission dialog: left/right choose, y allow, a always, n deny, enter
  // confirms, esc denies.
  { action: 'dialog:left', context: 'dialog', key: 'left' },
  { action: 'dialog:right', context: 'dialog', key: 'right' },
  { action: 'dialog:allow', context: 'dialog', key: 'y' },
  { action: 'dialog:always', context: 'dialog', key: 'a' },
  { action: 'dialog:deny', context: 'dialog', key: 'n' },
  { action: 'dialog:commit', context: 'dialog', key: 'return' },
  { action: 'dialog:cancel', context: 'dialog', key: 'escape' },
];

/**
 * Canonicalize an ink (input, key) event into a key id. Modifier prefixes
 * win, then named keys, then the literal character.
 */
export function strokeToKey(input: string, key: KeyInfo): string {
  if (key.ctrl && input.length === 1 && /[a-z]/.test(input)) return `ctrl+${input}`;
  if (key.escape) return 'escape';
  if (key.return) return 'return';
  if (key.tab) return 'tab';
  if (key.backspace) return 'backspace';
  if (key.delete) return 'delete';
  if (key.upArrow) return 'up';
  if (key.downArrow) return 'down';
  if (key.leftArrow) return 'left';
  if (key.rightArrow) return 'right';
  return input.length > 0 ? input : '';
}

export function resolveAction(
  bindings: readonly BindingRule[],
  contexts: readonly KeybindingContextName[],
  keyId: string,
): string | null {
  if (keyId.length === 0) return null;
  const order: KeybindingContextName[] = [...contexts, 'global'];
  for (const ctx of order) {
    const hit = bindings.find((b) => b.context === ctx && b.key === keyId);
    if (hit !== undefined) return hit.action;
  }
  return null;
}
