import { afterEach, describe, expect, test } from 'bun:test';
import * as fs from 'fs';
import * as os from 'os';
import * as path from 'path';
import { configPath, readThemeSetting, writeThemeSetting } from '../../state/themeStore.js';

let tmpDir: string;

function useTmpConfig(): string {
  tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'brain-theme-store-'));
  const p = path.join(tmpDir, 'config.json');
  process.env.BRAIN_CONFIG_PATH = p;
  return p;
}

afterEach(() => {
  delete process.env.BRAIN_CONFIG_PATH;
  if (tmpDir) fs.rmSync(tmpDir, { recursive: true, force: true });
});

describe('themeStore', () => {
  test('missing file resolves to auto', () => {
    useTmpConfig();
    expect(readThemeSetting()).toBe('auto');
  });

  test('reads a valid setting and preserves foreign keys on write', () => {
    const p = useTmpConfig();
    fs.writeFileSync(p, JSON.stringify({ theme: 'light', editorMode: 'vim', nested: { a: 1 } }));
    expect(readThemeSetting()).toBe('light');
    writeThemeSetting('light-daltonized');
    const doc = JSON.parse(fs.readFileSync(p, 'utf8'));
    expect(doc.theme).toBe('light-daltonized');
    expect(doc.editorMode).toBe('vim'); // preserved
    expect(doc.nested).toEqual({ a: 1 }); // preserved
  });

  test('legacy dark-ansi/light-ansi aliases map onto modern themes', () => {
    const p = useTmpConfig();
    fs.writeFileSync(p, JSON.stringify({ theme: 'dark-ansi' }));
    expect(readThemeSetting()).toBe('dark');
    fs.writeFileSync(p, JSON.stringify({ theme: 'light-ansi' }));
    expect(readThemeSetting()).toBe('light');
  });

  test('invalid JSON and unknown values fall back to auto without throwing', () => {
    const p = useTmpConfig();
    fs.writeFileSync(p, '{not json');
    expect(readThemeSetting()).toBe('auto');
    fs.writeFileSync(p, JSON.stringify({ theme: 'neon-purple' }));
    expect(readThemeSetting()).toBe('auto');
  });

  test('write creates parent directories and round-trips', () => {
    const p = useTmpConfig();
    fs.rmSync(path.dirname(p), { recursive: true, force: true });
    writeThemeSetting('dark');
    expect(readThemeSetting()).toBe('dark');
    expect(configPath()).toBe(p);
  });
});
