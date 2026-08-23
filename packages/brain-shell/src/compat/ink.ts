/**
 * Single import point for terminal primitives. Stock MIT-licensed Ink.
 * Anything stock Ink lacks lands in ./hooks or ./text — never vendor paths.
 */
import { useInput as _useInput } from 'ink';
export {
  Box,
  Text,
  Static,
  Newline,
  Spacer,
  Transform,
  useInput,
  usePaste,
  useApp,
  useStdin,
  useStdout,
  useStderr,
  useFocus,
  useCursor,
  render,
} from 'ink';
export type Key = Parameters<Parameters<typeof _useInput>[0]>[0];

export { Ansi } from './AnsiText.js';
export { usePreviewTheme, useTheme, useThemeSetting } from '../state/themeContext.js';
export { useTerminalFocus } from './focus.js';
export { createRoot } from './createRoot.js';

/**
 * Display width of a string: ANSI-stripped, East-Asian-wide aware (2 cols),
 * code-point counted otherwise. Zero-width joiner sequences are counted as
 * their base characters — acceptable for shell UI text; revisit if emoji
 * clusters matter in rendered output.
 */
const ANSI_RE = /\x1b\[[0-9;]*[A-Za-z]/g;
const WIDE_RE =
  /[\u{1100}-\u{115F}\u{2E80}-\u{303E}\u{3041}-\u{33FF}\u{3400}-\u{4DBF}\u{4E00}-\u{9FFF}\u{F900}-\u{FAFF}\u{FE30}-\u{FE6F}\u{FF00}-\u{FF60}\u{FFE0}-\u{FFE6}\u{1F300}-\u{1F64F}\u{1F900}-\u{1F9FF}]/u;

export function stringWidth(input: string): number {
  const clean = input.replace(ANSI_RE, '');
  let width = 0;
  for (const ch of clean) width += WIDE_RE.test(ch) ? 2 : 1;
  return width;
}
