import React from 'react';
import { render } from 'ink';
import { SocketClient } from './services/SocketClient';
import { REPL } from './screens/REPL';
import { ThemeProvider, ThemeType } from './components/design-system';
import path from 'path';

// Parse command line arguments for theme override
const args = process.argv.slice(2);
let themeOverride: string | undefined = process.env.BRAIN_THEME;

const themeArgIndex = args.indexOf('--theme');
if (themeArgIndex !== -1 && args[themeArgIndex + 1]) {
  themeOverride = args[themeArgIndex + 1];
} else {
  const tArgIndex = args.indexOf('-t');
  if (tArgIndex !== -1 && args[tArgIndex + 1]) {
    themeOverride = args[tArgIndex + 1];
  }
}

// Validate theme override type
const validThemes: ThemeType[] = [
  'dark',
  'light',
  'dark-daltonized',
  'light-daltonized',
  'dark-ansi',
  'light-ansi',
];

const selectedTheme: ThemeType | undefined = 
  themeOverride && validThemes.includes(themeOverride as ThemeType)
    ? (themeOverride as ThemeType)
    : undefined;

// Dynamically resolve UDS socket path under ~/.brain/
const homeDir = process.env.HOME || '/tmp';
const socketPath = process.env.BRAIN_SOCKET_PATH || path.join(homeDir, '.brain', 'daemon.sock');

const client = new SocketClient(socketPath);

// Start the Ink React application
render(
  <ThemeProvider defaultTheme={selectedTheme}>
    <REPL client={client} />
  </ThemeProvider>
);
