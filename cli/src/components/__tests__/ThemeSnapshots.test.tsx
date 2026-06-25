import React from 'react';
import { expect, test, afterEach } from 'bun:test';
import { EventEmitter } from 'events';
import { render as inkRender, cleanup } from 'ink-testing-library';
import { FocusManager } from '../../services/FocusManager';

if (!(EventEmitter.prototype as any).ref) {
  (EventEmitter.prototype as any).ref = function() { return this; };
}
if (!(EventEmitter.prototype as any).unref) {
  (EventEmitter.prototype as any).unref = function() { return this; };
}

afterEach(() => {
  cleanup();
  FocusManager.reset();
});

import {
  ThemeProvider,
  ThemeType,
  Spinner,
  Divider,
  StatusLine,
  Panel,
  Card,
  SuccessText,
  SuccessBadge,
  Alert,
  Progress,
} from '../design-system';
import { REPL } from '../../screens/REPL';

// Helper wrapper to patch stdin
const render = (tree: React.ReactNode) => {
  return inkRender(tree);
};

const themes: ThemeType[] = [
  'dark',
  'light',
  'dark-daltonized',
  'light-daltonized',
  'dark-ansi',
  'light-ansi',
];

// Mock SocketClient for REPL
class MockSocketClient {
  logCallbacks: any[] = [];
  msgCallbacks: any[] = [];
  connect() {}
  onLog(cb: any) {
    this.logCallbacks.push(cb);
    return () => {};
  }
  onMessage(cb: any) {
    this.msgCallbacks.push(cb);
    return () => {};
  }
  send() {}
}

// 1. Component Snapshots across all six themes
for (const themeName of themes) {
  test(`Components render correctly in theme: ${themeName}`, () => {
    const { lastFrame } = render(
      <ThemeProvider defaultTheme={themeName}>
        <Panel title="System Stats">
          <Spinner label="Scanning files..." />
          <Divider title="Details" />
          <Card>
            <SuccessText>All subsystems operational</SuccessText>
            <SuccessBadge>OK</SuccessBadge>
          </Card>
          <Alert severity="warning" title="STORAGE LIMIT">
            Disk utilization exceeds 85%.
          </Alert>
          <Progress percent={45} />
          <StatusLine />
        </Panel>
      </ThemeProvider>
    );
    expect(lastFrame()).toMatchSnapshot();
  });
}

// 2. Full REPL Screen Snapshots across all six themes
for (const themeName of themes) {
  test(`Full REPL screen renders correctly in theme: ${themeName}`, () => {
    const client = new MockSocketClient() as any;
    const { lastFrame } = render(
      <ThemeProvider defaultTheme={themeName}>
        <REPL client={client} />
      </ThemeProvider>
    );
    expect(lastFrame()).toMatchSnapshot();
  });
}
