import { describe, test, expect, beforeAll, afterAll } from 'bun:test';
import * as fs from 'fs';
import * as path from 'path';
import * as os from 'os';
import * as child_process from 'child_process';

const BRAIN_SHELL_DIR = path.resolve(import.meta.dir, '../..');
const PRELOAD_PATH = path.join(BRAIN_SHELL_DIR, 'src', 'preload.ts');
const MAIN_PATH = path.join(BRAIN_SHELL_DIR, 'src', 'main.tsx');

describe('Phase 6.3: Clean-Machine / Pristine Boot Verification', () => {
  const tempHomeDir = path.join(os.tmpdir(), `clean_home_${Date.now()}_${Math.random().toString(36).slice(2, 6)}`);

  beforeAll(() => {
    if (!fs.existsSync(tempHomeDir)) {
      fs.mkdirSync(tempHomeDir, { recursive: true });
    }
  });

  afterAll(() => {
    if (fs.existsSync(tempHomeDir)) {
      try {
        fs.rmSync(tempHomeDir, { recursive: true, force: true });
      } catch {}
    }
  });

  test('Clean-Machine Invariant 1: Pristine HOME starts cleanly with zero ~/.claude configuration', () => {
    // Assert tempHomeDir is completely empty
    expect(fs.readdirSync(tempHomeDir).length).toBe(0);

    const env = {
      ...process.env,
      HOME: tempHomeDir,
      USERPROFILE: tempHomeDir,
      NODE_ENV: 'production',
      DISABLE_AUTOUPDATER: '1',
      TERM: 'xterm-256color',
    };

    // Run quick version/help or bare boot probe in isolated clean HOME
    const proc = child_process.spawnSync(
      'bun',
      ['run', '--preload', PRELOAD_PATH, MAIN_PATH, '--version'],
      {
        cwd: BRAIN_SHELL_DIR,
        env,
        timeout: 5000,
        encoding: 'utf8',
      }
    );

    // Process must exit cleanly with code 0
    expect(proc.status).toBe(0);
  });

  test('Clean-Machine Invariant 2: Zero Anthropic login or authentication prompts are resurrected', () => {
    const env = {
      ...process.env,
      HOME: tempHomeDir,
      USERPROFILE: tempHomeDir,
      NODE_ENV: 'production',
      DISABLE_AUTOUPDATER: '1',
      TERM: 'xterm-256color',
    };

    const proc = child_process.spawnSync(
      'bun',
      ['run', '--preload', PRELOAD_PATH, MAIN_PATH, '--help'],
      {
        cwd: BRAIN_SHELL_DIR,
        env,
        timeout: 5000,
        encoding: 'utf8',
      }
    );

    const output = (proc.stdout || '') + (proc.stderr || '');
    expect(proc.status).toBe(0);
    // Must NOT complain about missing ANTHROPIC_API_KEY
    expect(output).not.toContain('Missing ANTHROPIC_API_KEY');
    expect(output).not.toContain('Please run /login');
  });
});
