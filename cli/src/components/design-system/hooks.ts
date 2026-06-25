import { useContext } from 'react';
import { ThemeContext, ThemeContextProps } from './ThemeProvider';

export const useTheme = (): ThemeContextProps => {
  return useContext(ThemeContext);
};
