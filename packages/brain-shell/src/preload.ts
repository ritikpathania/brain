// Brain shell preload: runtime hygiene and system-theme detection only.
// Module resolution needs no redirects — every import is Brain-owned.

// Production build selection must happen before react is imported.
process.env.NODE_ENV = 'production';

const cargoBin = `${require('os').homedir()}/.cargo/bin`;
if (!process.env.PATH?.includes(cargoBin)) {
  process.env.PATH = `${cargoBin}:${process.env.PATH || ''}`;
}

if (process.env.BRAIN_CALLER_CWD) {
  try {
    if (require('fs').existsSync(process.env.BRAIN_CALLER_CWD)) {
      process.chdir(process.env.BRAIN_CALLER_CWD);
    }
  } catch {}
}

(globalThis as any).__BRAIN_PRELOAD_LOADED = true;

process.on('uncaughtException', (err) => {
  require('fs').appendFileSync('/tmp/brain_crash.log', 'UNCAUGHT: ' + String(err?.stack || err) + '\n');
  process.stderr.write('Uncaught error in Brain Shell: ' + String(err?.stack || err) + '\n');
});
process.on('unhandledRejection', (err) => {
  require('fs').appendFileSync('/tmp/brain_crash.log', 'UNHANDLED_REJECTION: ' + String((err as any)?.stack || err) + '\n');
});

// AUTO_THEME: detect the terminal's light/dark and expose it for
// contracts/theme.ts (COLORFGBG is its fallback heuristic).
try {
  // COLORFGBG is "fg;background"; background 0–6 is dark, 7/15 light.
  const bg = Number((process.env.COLORFGBG ?? '').split(';')[1]);
  (globalThis as any).__BRAIN_SYSTEM_THEME = Number.isFinite(bg) && bg >= 7 ? 'light' : 'dark';
} catch {
  (globalThis as any).__BRAIN_SYSTEM_THEME = 'dark';
}
