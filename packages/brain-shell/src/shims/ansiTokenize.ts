import { ansiCodesToString } from '../../node_modules/@alcalzone/ansi-tokenize/build/ansiCodes.js';
import { undoAnsiCodes } from '../../node_modules/@alcalzone/ansi-tokenize/build/undo.js';
import { reduceAnsiCodes, reduceAnsiCodesIncremental } from '../../node_modules/@alcalzone/ansi-tokenize/build/reduce.js';
import { styledCharsFromTokens } from '../../node_modules/@alcalzone/ansi-tokenize/build/styledChars.js';
import { tokenize } from '../../node_modules/@alcalzone/ansi-tokenize/build/tokenize.js';

export type AnsiCode = {
  type: 'ansi';
  code: string;
  endCode: string;
};

export type StyledChar = {
  type: 'char';
  value: string;
  fullWidth: boolean;
  styles?: AnsiCode[];
};

export type Token = AnsiCode | { type: 'char'; value: string; fullWidth: boolean };

export {
  ansiCodesToString,
  reduceAnsiCodes,
  reduceAnsiCodesIncremental,
  undoAnsiCodes,
  styledCharsFromTokens,
  tokenize,
};

/**
 * Corrected diffAnsiCodes implementation.
 *
 * In standard ANSI terminals, SGR 1m (bold) and SGR 2m (faint/dim) share the reset code
 * SGR 22m (normal intensity). However, sending SGR 2m does NOT clear SGR 1m in terminal
 * screen buffers. Therefore, transitioning between bold and dim (or any styles sharing an endCode)
 * requires explicitly emitting the undo code (e.g. SGR 22m) whenever the starting code is not
 * in the destination set.
 */
export function diffAnsiCodes(from: AnsiCode[], to: AnsiCode[]): AnsiCode[] {
  const startCodesInTo = new Set(to.map((code) => code.code));
  const startCodesInFrom = new Set(from.map((code) => code.code));

  // Undo any code in 'from' whose exact code is not present in 'to'
  const toUndo = from.filter((code) => !startCodesInTo.has(code.code));
  // Add any code in 'to' that wasn't already active in 'from'
  const toAdd = to.filter((code) => !startCodesInFrom.has(code.code));

  return [
    ...undoAnsiCodes(toUndo),
    ...toAdd,
  ];
}
