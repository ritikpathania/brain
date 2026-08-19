const scriptIndex = process.argv.findIndex(arg => arg.endsWith('main.tsx') || arg.endsWith('main.js') || arg.endsWith('main.ts'));
if (scriptIndex > 1) {
  process.argv = [process.argv[0], process.argv[scriptIndex], ...process.argv.slice(scriptIndex + 1)];
}

const { main } = await import('../vendor/claude/main.js');
await main();
