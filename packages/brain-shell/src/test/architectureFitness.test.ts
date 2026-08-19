import * as fs from 'fs';
import * as path from 'path';
import { describe, it, expect } from 'bun:test';

const BRAIN_SHELL_DIR = path.resolve(import.meta.dir, '../..');
const MAIN_FILE = path.join(BRAIN_SHELL_DIR, 'src', 'main.tsx');
const PRELOAD_FILE = path.join(BRAIN_SHELL_DIR, 'src', 'preload.ts');
const VENDOR_DIR = path.join(BRAIN_SHELL_DIR, 'vendor', 'claude');

describe('Phase 3 Architecture & Invariant Protections', () => {
  it('enforces host entrypoint does not construct or fabricate Claude application state', () => {
    const mainContent = fs.readFileSync(MAIN_FILE, 'utf8');
    
    // Host must directly delegate to upstream entrypoint
    expect(mainContent.includes("import { main } from '../vendor/claude/main.js';") || mainContent.includes("await import('../vendor/claude/main.js')")).toBe(true);
    expect(mainContent).toContain('await main();');

    // Host must NOT import presentation components or stores directly
    const forbiddenImports = [
      'AppStateStore',
      'createStatsStore',
      'FpsTracker',
      'PromptInput',
      'LogoV2',
      '<App',
      '<REPL',
      'createRoot',
    ];
    for (const item of forbiddenImports) {
      expect(mainContent).not.toContain(item);
    }
  });

  it('enforces preload shims use real dependencies without fake string or filter stubs', () => {
    const preloadContent = fs.readFileSync(PRELOAD_FILE, 'utf8');

    // Fake stubs must NOT exist in preload
    expect(preloadContent).not.toContain('class Fuse');
    expect(preloadContent).not.toContain('toDataURL');
    expect(preloadContent).not.toContain('/tmp/brain_claude_config');
  });

  it('enforces zero Brain, Rust, or UDS dependencies inside vendor/claude and brain-shell', () => {
    const mainContent = fs.readFileSync(MAIN_FILE, 'utf8');
    const preloadContent = fs.readFileSync(PRELOAD_FILE, 'utf8');

    expect(mainContent).not.toContain('brain-services');
    expect(mainContent).not.toContain('brain-domain');
    expect(mainContent).not.toContain('BrainUiBridge');
    expect(mainContent).not.toContain('BrainPresentationModel');
    expect(preloadContent).not.toContain('brain-services');
  });
});
