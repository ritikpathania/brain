/** Pure keybinding translation: ink's (input, key) → editor command. */

export interface KeyInfo {
  upArrow?: boolean;
  downArrow?: boolean;
  leftArrow?: boolean;
  rightArrow?: boolean;
  return?: boolean;
  escape?: boolean;
  ctrl?: boolean;
  meta?: boolean;
  shift?: boolean;
  backspace?: boolean;
  delete?: boolean;
  tab?: boolean;
}

export type KeyCommand =
  | { type: 'insert'; text: string }
  | { type: 'backspace' }
  | { type: 'left' }
  | { type: 'right' }
  | { type: 'home' }
  | { type: 'end' }
  | { type: 'kill_to_end' }
  | { type: 'kill_to_start' }
  | { type: 'delete_word_back' }
  | { type: 'undo' }
  | { type: 'history_up' }
  | { type: 'history_down' }
  | { type: 'newline' }
  | { type: 'submit' }
  | { type: 'abort' }
  | { type: 'exit' }
  | { type: 'noop' };

export function translateKey(input: string, key: KeyInfo): KeyCommand {
  if (key.escape) return { type: 'abort' };
  if (key.return) {
    return key.shift ? { type: 'newline' } : { type: 'submit' };
  }
  if (key.upArrow) return { type: 'history_up' };
  if (key.downArrow) return { type: 'history_down' };
  if (key.leftArrow) return { type: 'left' };
  if (key.rightArrow) return { type: 'right' };
  if (key.backspace || key.delete) return { type: 'backspace' };
  if (key.ctrl) {
    switch (input) {
      case 'a': return { type: 'home' };
      case 'e': return { type: 'end' };
      case 'k': return { type: 'kill_to_end' };
      case 'u': return { type: 'kill_to_start' };
      case 'w': return { type: 'delete_word_back' };
      case 'z':
      case '_': return { type: 'undo' };
      case 'c': return { type: 'exit' };
      default: break;
    }
  }
  if (input && !key.ctrl && !key.meta) return { type: 'insert', text: input };
  return { type: 'noop' };
}
