/**
 * Theme persistence: the `theme` key of the user's brain config file.
 * Original, minimal surface — read/merge-write only, tolerant of missing
 * files, bad JSON, and legacy values. Other keys pass through untouched.
 */
import * as fs from 'fs';
import * as path from 'path';
import * as os from 'os';
import type { ThemeSetting } from '../contracts/theme.js';

const VALID: readonly string[] = [
  'auto',
  'dark',
  'light',
  'dark-daltonized',
  'light-daltonized',
];

/** Legacy values kept readable so old configs still resolve. */
const LEGACY_ALIASES: Record<string, ThemeSetting> = {
  'dark-ansi': 'dark',
  'light-ansi': 'light',
};

export function configPath(): string {
  if (process.env.BRAIN_CONFIG_PATH) return path.resolve(process.env.BRAIN_CONFIG_PATH);
  return path.join(os.homedir(), '.brain', 'config.json');
}

export function readThemeSetting(): ThemeSetting {
  try {
    const parsed = JSON.parse(fs.readFileSync(configPath(), 'utf8')) as { theme?: unknown };
    const t = parsed && typeof parsed === 'object' ? parsed.theme : undefined;
    if (typeof t === 'string' && VALID.includes(t)) return t as ThemeSetting;
    if (typeof t === 'string' && LEGACY_ALIASES[t] !== undefined) return LEGACY_ALIASES[t]!;
  } catch {
    // missing file / bad JSON → default below
  }
  return 'auto';
}

export function writeThemeSetting(setting: ThemeSetting): void {
  let doc: Record<string, unknown> = {};
  try {
    const parsed = JSON.parse(fs.readFileSync(configPath(), 'utf8')) as unknown;
    if (parsed && typeof parsed === 'object') doc = parsed as Record<string, unknown>;
  } catch {
    // start a fresh document
  }
  doc.theme = setting;
  fs.mkdirSync(path.dirname(configPath()), { recursive: true });
  fs.writeFileSync(configPath(), JSON.stringify(doc, null, 2) + '\n');
}
