import { useCallback, useContext, useLayoutEffect, useRef } from 'react';
import { TerminalSizeContext } from '../../vendor/claude/ink/components/TerminalSizeContext.js';
import type { DOMElement } from '../../vendor/claude/ink/dom.js';

type ViewportEntry = {
  isVisible: boolean;
};

function computeVisibility(
  element: DOMElement | null,
  terminalSize: { columns: number; rows: number } | null
): boolean | null {
  if (!element?.yogaNode || !terminalSize) return null;
  const height = element.yogaNode.getComputedHeight();
  const rows = terminalSize.rows;
  let absoluteTop = element.yogaNode.getComputedTop();
  let parent: DOMElement | undefined = element.parentNode;
  let root = element.yogaNode;
  while (parent) {
    if (parent.yogaNode) {
      absoluteTop += parent.yogaNode.getComputedTop();
      root = parent.yogaNode;
    }
    if (parent.scrollTop) absoluteTop -= parent.scrollTop as number;
    parent = parent.parentNode;
  }
  const screenHeight = root.getComputedHeight();
  const bottom = absoluteTop + height;
  const cursorRestoreScroll = screenHeight > rows ? 1 : 0;
  const viewportY = Math.max(0, screenHeight - rows) + cursorRestoreScroll;
  const viewportBottom = viewportY + rows;
  if (height === 0) return absoluteTop >= viewportY && absoluteTop < viewportBottom;
  return bottom > viewportY && absoluteTop < viewportBottom;
}

export function useTerminalViewport(): [
  ref: (element: DOMElement | null) => void,
  entry: ViewportEntry,
  getFresh: () => boolean,
  computeFresh: () => boolean | null,
] {
  const terminalSize = useContext(TerminalSizeContext);
  const elementRef = useRef<DOMElement | null>(null);
  const entryRef = useRef<ViewportEntry>({ isVisible: true });

  const setElement = useCallback((el: DOMElement | null) => {
    elementRef.current = el;
  }, []);

  function getFresh(): boolean {
    const fresh = computeVisibility(elementRef.current, terminalSize);
    if (fresh === null) return entryRef.current.isVisible;
    if (fresh !== entryRef.current.isVisible) {
      entryRef.current = { isVisible: fresh };
    }
    return fresh;
  }

  const getFreshRef = useRef(getFresh);
  getFreshRef.current = getFresh;
  const getFreshStable = useCallback(() => getFreshRef.current(), []);

  const terminalSizeRef = useRef(terminalSize);
  terminalSizeRef.current = terminalSize;
  const computeFreshStable = useCallback(
    () => computeVisibility(elementRef.current, terminalSizeRef.current),
    []
  );

  useLayoutEffect(() => {
    getFresh();
  });

  return [setElement, entryRef.current, getFreshStable, computeFreshStable];
}
