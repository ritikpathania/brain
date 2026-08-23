/**
 * React context for live theme state. Components read tokens via useTheme();
 * ThemePicker (Inc 3) previews via usePreviewTheme() without process restarts.
 */
import * as React from 'react';
import type { ThemeName, ThemeSetting } from '../contracts/theme.js';
import { resolveThemeSetting } from '../contracts/theme.js';
import { PALETTES, type BrainTokens } from './palettes.js';

export interface ThemeContextValue {
  setting: ThemeSetting;
  themeName: ThemeName;
  tokens: BrainTokens;
}

const ThemeReactContext = React.createContext<ThemeContextValue>({
  setting: 'dark',
  themeName: 'dark',
  tokens: PALETTES.dark,
});

function valueFor(setting: ThemeSetting): ThemeContextValue {
  const themeName = resolveThemeSetting(setting);
  return { setting, themeName, tokens: PALETTES[themeName] };
}

export function ThemeProvider({
  setting = 'dark',
  children,
}: {
  setting?: ThemeSetting;
  children: React.ReactNode;
}): React.ReactElement {
  const [value, setValue] = React.useState(() => valueFor(setting));
  const applySetting = React.useCallback((next: ThemeSetting) => {
    setValue(valueFor(next));
  }, []);
  // Expose an imperative setter for settings UI without re-mounting the tree.
  const ctx = React.useMemo(() => Object.assign(value, { setSetting: applySetting }), [value, applySetting]);
  return <ThemeReactContext.Provider value={ctx}>{children}</ThemeReactContext.Provider>;
}

/** Active theme tokens + name. */
export function useTheme(): ThemeContextValue {
  return React.useContext(ThemeReactContext);
}

/** Same as useTheme but marks reads that tolerate transient preview states. */
export function usePreviewTheme(): ThemeContextValue {
  return React.useContext(ThemeReactContext);
}

/** Current resolved theme name only. */
export function useThemeSetting(): ThemeName {
  return React.useContext(ThemeReactContext).themeName;
}
