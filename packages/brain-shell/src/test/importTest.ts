import { ThemeProvider, useTheme } from '../../vendor/claude/components/design-system/ThemeProvider.js';
import { THEME_NAMES } from '../../vendor/claude/utils/theme.js';
import { LogoV2 } from '../../vendor/claude/components/LogoV2/LogoV2.js';
import { FullscreenLayout } from '../../vendor/claude/components/FullscreenLayout.js';

console.log('Direct Claude vendor import successful!');
console.log('THEME_NAMES:', THEME_NAMES);
console.log('LogoV2 is loaded:', typeof LogoV2 === 'function' || typeof LogoV2 === 'object');
console.log('FullscreenLayout is loaded:', typeof FullscreenLayout === 'function');
