import React, { createContext, useState } from 'react';
import { ThemeType, themes } from './themes';
import { Theme, ColorToken, SpacingToken, IconToken, BorderToken, TypographyToken, TypographyStyle } from './tokens';

export interface ThemeContextProps {
  themeType: ThemeType;
  theme: Theme;
  setTheme: (type: ThemeType) => void;
  color: (token: ColorToken | string) => string;
  spacing: (token: SpacingToken) => number;
  icon: (token: IconToken) => string;
  border: (token: BorderToken) => 'single' | 'double' | 'round' | 'classic';
  typography: (token: TypographyToken) => TypographyStyle;
}

export const ThemeContext = createContext<ThemeContextProps>({
  themeType: 'dark',
  theme: themes.dark,
  setTheme: () => {},
  color: (t) => t,
  spacing: () => 0,
  icon: () => '',
  border: () => 'round',
  typography: () => ({}),
});

const detectIsLightBackground = (): boolean => {
  const colorfgbg = process.env.COLORFGBG;
  if (colorfgbg) {
    const parts = colorfgbg.split(';');
    if (parts.length > 1) {
      const bgIndex = parseInt(parts[parts.length - 1], 10);
      if (!isNaN(bgIndex) && bgIndex >= 8 && bgIndex !== 15) {
        return true;
      }
    }
  }
  return false;
};

export const ThemeProvider: React.FC<{ children: React.ReactNode; defaultTheme?: ThemeType }> = ({
  children,
  defaultTheme,
}) => {
  const getInitialTheme = (): ThemeType => {
    if (defaultTheme) return defaultTheme;
    return detectIsLightBackground() ? 'light' : 'dark';
  };

  const [themeType, setThemeType] = useState<ThemeType>(getInitialTheme());

  const setTheme = (type: ThemeType) => {
    setThemeType(type);
  };

  const activeTheme = themes[themeType] || themes.dark;

  // Precomputed helper functions
  const color = (token: ColorToken | string): string => {
    if (token in activeTheme.colors) {
      return activeTheme.colors[token as ColorToken];
    }
    return token;
  };

  const spacing = (token: SpacingToken): number => {
    return activeTheme.spacing[token] ?? 0;
  };

  const icon = (token: IconToken): string => {
    return activeTheme.icons[token] ?? '';
  };

  const border = (token: BorderToken) => {
    return activeTheme.borders[token] ?? 'round';
  };

  const typography = (token: TypographyToken): TypographyStyle => {
    return activeTheme.typography[token] ?? {};
  };

  return (
    <ThemeContext.Provider value={{
      themeType,
      theme: activeTheme,
      setTheme,
      color,
      spacing,
      icon,
      border,
      typography,
    }}>
      {children}
    </ThemeContext.Provider>
  );
};
