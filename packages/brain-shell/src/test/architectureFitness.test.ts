import * as fs from 'fs';
import * as path from 'path';
import { describe, it, expect } from 'bun:test';

const BRAIN_SHELL_DIR = path.resolve(import.meta.dir, '../..');
const MAIN_FILE = path.join(BRAIN_SHELL_DIR, 'src', 'main.tsx');
const PRELOAD_FILE = path.join(BRAIN_SHELL_DIR, 'src', 'preload.ts');

describe('Phase 3 Architecture & Invariant Protections', () => {
  it('enforces host entrypoint boots the Brain shell with zero vendor reach', () => {
    const mainContent = fs.readFileSync(MAIN_FILE, 'utf8');

    expect(mainContent).toContain('await main();');
    expect(mainContent).toContain('AppShell');
    // The entrypoint mounts Brain's own shell; the vendored tree must be
    // unreachable from it entirely.
    expect(mainContent.includes('vendor')).toBe(false);
  });

  it('enforces preload stays vendor-free and product-identity-free', () => {
    const preloadContent = fs.readFileSync(PRELOAD_FILE, 'utf8');

    expect(preloadContent.includes('vendor')).toBe(false);
    expect(preloadContent).toContain('__BRAIN_PRELOAD_LOADED');
    expect(preloadContent).toContain('__BRAIN_SYSTEM_THEME');
    // No upstream product identity leaks through globals or env.
    expect(preloadContent).not.toContain('MACRO');
    expect(preloadContent.toLowerCase()).not.toContain('claude');
    expect(preloadContent.toLowerCase()).not.toContain('anthropic');
    expect(preloadContent).not.toContain('brain-services');
  });
});
