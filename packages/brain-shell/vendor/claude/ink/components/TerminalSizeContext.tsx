import { createContext } from 'react';
export type TerminalSize = {
  columns: number;
  rows: number;
};
export const TerminalSizeContext = ((globalThis as any).__TERMINAL_SIZE_CONTEXT__ ||= createContext<TerminalSize | null>(null));