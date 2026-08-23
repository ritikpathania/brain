// Preload first, statically: it must set NODE_ENV before react selects a
// build. When bunfig already preloaded it, the module cache dedupes.
import './preload.js';
import { render } from './compat/ink.js';
import * as React from 'react';
import { AppShell } from './ui/shell/AppShell.js';

export async function main(): Promise<void> {
  const app = render(React.createElement(AppShell), { patchConsole: false });
  process.on('SIGINT', () => { app.unmount(); process.exit(0); });
}

await main();
