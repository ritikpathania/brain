import React from 'react';
import { createRoot } from '../../vendor/claude/ink.js';
import { App } from '../../vendor/claude/components/App.js';
import { REPL } from '../../vendor/claude/screens/REPL.js';
import { getDefaultAppState } from '../../vendor/claude/state/AppStateStore.js';
import { enableConfigs } from '../../vendor/claude/utils/config.js';
import { createStatsStore } from '../../vendor/claude/context/stats.js';
import { FpsTracker } from '../../vendor/claude/utils/fpsTracker.js';

let fullOutput = '';
const mockStdout = {
  write: (str: string) => { fullOutput += str; return true; },
  on: () => {},
  once: () => {},
  emit: () => {},
  removeListener: () => {},
  columns: 80,
  rows: 24,
};

async function test() {
  enableConfigs();

  const fpsTracker = new FpsTracker();
  const stats = createStatsStore();
  const initialState = getDefaultAppState();

  const root = await createRoot({
    stdout: mockStdout as any,
    patchConsole: false,
  });

  const replProps = {
    commands: [],
    debug: false,
    initialTools: [],
    thinkingConfig: { mode: 'off' as const },
    onBeforeQuery: async () => false,
  };

  root.render(
    <App
      getFpsMetrics={() => fpsTracker.getMetrics()}
      stats={stats}
      initialState={initialState}
    >
      <REPL {...replProps} />
    </App>
  );

  await new Promise((r) => setTimeout(r, 600));
  root.unmount();

  console.log('--- RENDER OUTPUT SAMPLE ---');
  console.log('Output length:', fullOutput.length);
  console.log('Output raw (first 1000 chars):', JSON.stringify(fullOutput.slice(0, 1000)));
}

test().catch(err => {
  console.error('Error during test:', err);
});
