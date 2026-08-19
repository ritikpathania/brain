import { feature } from 'bun:bundle';
import React, { createContext, useContext, useEffect, useMemo, useState } from 'react';
import useStdin from '../../vendor/claude/ink/hooks/use-stdin.js';
import { getGlobalConfig, saveGlobalConfig } from '../../vendor/claude/utils/config.js';
import { getSystemThemeName, type SystemTheme } from '../../vendor/claude/utils/systemTheme.js';
import type { ThemeName, ThemeSetting } from '../../vendor/claude/utils/theme.js';

export type ThemeContextValue = {
  themeSetting: ThemeSetting;
  setThemeSetting: (setting: ThemeSetting) => void;
  setPreviewTheme: (setting: ThemeSetting) => void;
  savePreview: () => void;
  cancelPreview: () => void;
  currentTheme: ThemeName;
};

const DEFAULT_THEME: ThemeName = 'dark';
export const ThemeContext = ((globalThis as any).__THEME_CONTEXT__ ||= createContext<ThemeContextValue>({
  themeSetting: DEFAULT_THEME,
  setThemeSetting: () => {},
  setPreviewTheme: () => {},
  savePreview: () => {},
  cancelPreview: () => {},
  currentTheme: DEFAULT_THEME,
}));

type Props = {
  children: React.ReactNode;
  initialState?: ThemeSetting;
  onThemeSave?: (setting: ThemeSetting) => void;
};

function defaultInitialTheme(): ThemeSetting {
  return getGlobalConfig().theme;
}

function defaultSaveTheme(setting: ThemeSetting): void {
  saveGlobalConfig((current) => ({
    ...current,
    theme: setting,
  }));
}

export function ThemeProvider({
  children,
  initialState,
  onThemeSave = defaultSaveTheme,
}: Props) {
  const [themeSetting, setThemeSetting] = useState(initialState ?? defaultInitialTheme);
  const [previewTheme, setPreviewTheme] = useState<ThemeSetting | null>(null);

  const [systemTheme, setSystemTheme] = useState<SystemTheme>(() =>
    (initialState ?? themeSetting) === 'auto' ? getSystemThemeName() : 'dark'
  );

  const activeSetting = previewTheme ?? themeSetting;
  const { internal_querier } = useStdin();

  useEffect(() => {
    if (feature('AUTO_THEME')) {
      if (activeSetting !== 'auto' || !internal_querier) return;
      let cleanup: (() => void) | undefined;
      let cancelled = false;
      void import('../../vendor/claude/utils/systemThemeWatcher.js').then(({ watchSystemTheme }) => {
        if (cancelled) return;
        cleanup = watchSystemTheme(internal_querier, setSystemTheme);
      });
      return () => {
        cancelled = true;
        cleanup?.();
      };
    }
  }, [activeSetting, internal_querier]);

  const currentTheme: ThemeName = activeSetting === 'auto' ? systemTheme : activeSetting;

  const value = useMemo<ThemeContextValue>(
    () => ({
      themeSetting,
      setThemeSetting: (newSetting: ThemeSetting) => {
        setThemeSetting(newSetting);
        setPreviewTheme(null);
        if (newSetting === 'auto') {
          setSystemTheme(getSystemThemeName());
        }
        onThemeSave?.(newSetting);
      },
      setPreviewTheme: (newSetting_0: ThemeSetting) => {
        setPreviewTheme(newSetting_0);
        if (newSetting_0 === 'auto') {
          setSystemTheme(getSystemThemeName());
        }
      },
      savePreview: () => {
        if (previewTheme !== null) {
          setThemeSetting(previewTheme);
          setPreviewTheme(null);
          onThemeSave?.(previewTheme);
        }
      },
      cancelPreview: () => {
        if (previewTheme !== null) {
          setPreviewTheme(null);
        }
      },
      currentTheme,
    }),
    [themeSetting, previewTheme, currentTheme, onThemeSave]
  );

  return (
    <ThemeContext.Provider value={value}>
      {children}
    </ThemeContext.Provider>
  );
}

export function useTheme(): [ThemeName, (setting: ThemeSetting) => void] {
  const { currentTheme, setThemeSetting } = useContext(ThemeContext);
  return [currentTheme, setThemeSetting];
}

export function useThemeSetting(): ThemeSetting {
  return useContext(ThemeContext).themeSetting;
}

export function usePreviewTheme() {
  const { setPreviewTheme, savePreview, cancelPreview } = useContext(ThemeContext);
  return { setPreviewTheme, savePreview, cancelPreview };
}
