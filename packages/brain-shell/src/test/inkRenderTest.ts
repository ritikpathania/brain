import React from 'react';
import { createRoot, Box, Text } from '../../vendor/claude/ink.js';
import { LogoV2 } from '../../vendor/claude/components/LogoV2/LogoV2.js';
import { FullscreenLayout } from '../../vendor/claude/components/FullscreenLayout.js';
import { ThemeProvider } from '../../vendor/claude/components/design-system/ThemeProvider.js';
import { Writable } from 'stream';

let output = '';
const customStdout = new Writable({
  write(chunk, encoding, callback) {
    output += chunk.toString();
    callback();
  }
});
(customStdout as any).columns = 80;
(customStdout as any).rows = 24;

async function run() {
  const root = await createRoot({ stdout: customStdout as any, isDirectConnect: false });
  
  root.render(
    <ThemeProvider initialState="dark">
      <Box flexDirection="column" width={80} height={24}>
        <FullscreenLayout
          scrollable={<LogoV2 />}
          bottom={<Text color="claude">Claude Code v2.1.232 Shell Host</Text>}
        />
      </Box>
    </ThemeProvider>
  );

  // Give React/Ink a tick to layout and render
  await new Promise((r) => setTimeout(r, 100));
  root.unmount();

  console.log('Ink render test passed! Output bytes:', output.length);
  console.log('Contains Claude brand:', output.includes('Claude') || output.length > 500);
}

run().catch((err) => {
  console.error('Render test failed:', err);
  process.exit(1);
});
