import { render, type Instance } from 'ink';

import type { BrainFrontendController } from '../../state/interactions.js';
import { defaultBrainTheme, type BrainTheme } from '../theme.js';
import { BrainTerminalApp } from './app.js';

export function renderBrainTerminal(
  controller: BrainFrontendController,
  theme: BrainTheme = defaultBrainTheme,
): Instance {
  return render(<BrainTerminalApp controller={controller} theme={theme} />, { exitOnCtrlC: false });
}
