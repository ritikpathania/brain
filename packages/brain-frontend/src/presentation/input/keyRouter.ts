import type { FrontendFocus } from '../../state/focus.js';

export type TerminalKey = Readonly<{
  input: string;
  return: boolean;
  escape: boolean;
  ctrl: boolean;
  shift: boolean;
  pageUp: boolean;
  pageDown: boolean;
  upArrow: boolean;
  downArrow: boolean;
  backspace: boolean;
  leftArrow?: boolean;
  rightArrow?: boolean;
  home?: boolean;
  end?: boolean;
}>;

export type TerminalIntent =
  | 'append'
  | 'newline'
  | 'submit'
  | 'cancel'
  | 'dismiss-overlay'
  | 'open-session-picker'
  | 'scroll-up'
  | 'scroll-bottom'
  | 'delete-backward'
  | 'picker-next'
  | 'picker-previous'
  | 'picker-select'
  | 'cursor-left'
  | 'cursor-right'
  | 'cursor-up'
  | 'cursor-down'
  | 'cursor-home'
  | 'cursor-end'
  | 'ignore';
// Explicit local interaction intents; none imply tool execution or backend retry.
export type Phase3Intent = 'permission-allow' | 'permission-deny' | 'dismiss-context' | 'toggle-tool-detail';

export function routeTerminalKey(key: TerminalKey, context: Readonly<{ focus: FrontendFocus; generating: boolean }>): TerminalIntent | Phase3Intent {
  if (context.focus === 'permission') {
    if (key.pageUp) return 'scroll-up';
    if (key.pageDown) return 'scroll-bottom';
    if (key.ctrl && key.input.toLowerCase() === 'c') return 'cancel';
    if (key.escape || (!key.ctrl && key.input.toLowerCase() === 'n')) return 'permission-deny';
    if (!key.ctrl && key.input.toLowerCase() === 'y') return 'permission-allow';
    return 'ignore';
  }
  if (context.focus === 'context') {
    if (key.escape || key.return || (key.ctrl && key.input.toLowerCase() === 'c')) return 'dismiss-context';
    return 'ignore';
  }
  if (context.focus === 'session-picker') {
    if (key.escape || (key.ctrl && key.input.toLowerCase() === 'c')) return 'dismiss-overlay';
    if (key.upArrow) return 'picker-previous';
    if (key.downArrow) return 'picker-next';
    if (key.return) return 'picker-select';
    return 'ignore';
  }
  if (key.pageUp) return 'scroll-up';
  if (key.pageDown) return 'scroll-bottom';
  if (key.ctrl && key.input.toLowerCase() === 't') return 'toggle-tool-detail';
  if (key.ctrl && key.input.toLowerCase() === 'o') return context.generating ? 'ignore' : 'open-session-picker';
  if ((key.escape || (key.ctrl && key.input.toLowerCase() === 'c')) && context.generating) return 'cancel';
  if ((key.return && key.shift) || (key.ctrl && key.input.toLowerCase() === 'j')) return context.generating ? 'ignore' : 'newline';
  if (key.return) return context.generating ? 'ignore' : 'submit';
  if (key.backspace && !context.generating) return 'delete-backward';
  if (!context.generating) {
    if (key.leftArrow) return 'cursor-left';
    if (key.rightArrow) return 'cursor-right';
    if (key.upArrow) return 'cursor-up';
    if (key.downArrow) return 'cursor-down';
    if (key.home) return 'cursor-home';
    if (key.end) return 'cursor-end';
  }
  if (context.generating || key.ctrl || key.escape || key.upArrow || key.downArrow) return 'ignore';
  return key.input.length > 0 ? 'append' : 'ignore';
}
