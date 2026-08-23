/**
 * Brain-owned theme vocabulary. Color values themselves stay in
 * adapter/BrainTheme.ts token maps; this module owns names, resolution,
 * and Brain-neutral presentation labels.
 */
import * as React from 'react';

export type ThemeName = 'dark' | 'light' | 'dark-daltonized' | 'light-daltonized';
export type ThemeSetting = ThemeName | 'auto';
export const THEME_NAMES: ThemeName[] = ['dark', 'light', 'dark-daltonized', 'light-daltonized'];
export type SystemTheme = 'dark' | 'light';

/** Preload AUTO_THEME detection writes this global; COLORFGBG is the fallback heuristic. */
export function getSystemThemeName(): SystemTheme {
  const g = globalThis as Record<string, unknown>;
  if (typeof g.__BRAIN_SYSTEM_THEME === 'string') return g.__BRAIN_SYSTEM_THEME as SystemTheme;
  if (process.env.BRAIN_THEME === 'light' || process.env.BRAIN_THEME === 'dark') {
    return process.env.BRAIN_THEME;
  }
  return 'dark';
}

export function resolveThemeSetting(setting: ThemeSetting): ThemeName {
  if (setting !== 'auto') return setting;
  return getSystemThemeName();
}

/** Model-setting label for status display. Brain-neutral: no vendor product names. */
export function renderModelSetting(model: string): string {
  return model;
}

const DEFAULT_MODEL = process.env.BRAIN_MODEL ?? 'brain-default';

/** Hook: current model label for the status line. */
export function useMainLoopModel(): string {
  const [model] = React.useState(DEFAULT_MODEL);
  return model;
}
