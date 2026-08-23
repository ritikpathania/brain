import { render, type RenderOptions, type Instance } from 'ink';
import * as React from 'react';

/**
 * Imperative root handle: replaces the current tree without stacking renders.
 * Mirrors the createRoot() consumption pattern used by shell entrypoints.
 */
export interface BrainRoot {
  render(element: React.ReactElement, options?: RenderOptions): Instance;
  unmount(): void;
}

export function createRoot(options?: RenderOptions): BrainRoot {
  let instance: Instance | null = null;
  return {
    render(element: React.ReactElement, renderOptions?: RenderOptions): Instance {
      instance?.unmount();
      instance = render(element, { ...options, ...renderOptions });
      return instance;
    },
    unmount(): void {
      instance?.unmount();
      instance = null;
    },
  };
}
