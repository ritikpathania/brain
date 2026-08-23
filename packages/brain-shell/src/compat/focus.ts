import * as React from 'react';

/**
 * Terminal focus tracking. Inc 0 reports focused=true; real DECSET 1004
 * focus-event tracking (ESC[I / ESC[O) lands with the input system in Inc 1,
 * where stdin raw mode is owned centrally.
 */
export function useTerminalFocus(): boolean {
  return React.useRef(true).current;
}
