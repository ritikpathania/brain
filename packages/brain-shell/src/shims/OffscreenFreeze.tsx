import React, { useContext, useRef } from 'react';
import { useTerminalViewport } from '../shims/useTerminalViewport.js';
import { Box } from '../../vendor/claude/ink.js';
import { InVirtualListContext } from '../../vendor/claude/components/messageActions.js';

type Props = {
  children: React.ReactNode;
};

/**
 * Freezes children when they scroll above the terminal viewport (into scrollback).
 *
 * Matches the vendor OffscreenFreeze semantics exactly:
 * - Uses `isVisible` from the entry ref (last useLayoutEffect-computed value), not a
 *   synchronous computeFresh() call that reads stale Yoga layout mid-render.
 * - When visible (or in virtual list), always updates cached.current.
 * - When offscreen, returns cached.current (stale but frozen element ref).
 */
export function OffscreenFreeze({ children }: Props): React.ReactNode {
  'use no memo';

  const inVirtualList = useContext(InVirtualListContext);
  const [ref, { isVisible }] = useTerminalViewport();
  const cached = useRef(children);
  // Virtual list has no terminal scrollback — the ScrollBox clips inside the
  // viewport, so there's nothing to freeze.
  if (isVisible || inVirtualList) {
    cached.current = children;
  }
  return <Box ref={ref}>{cached.current}</Box>;
}
