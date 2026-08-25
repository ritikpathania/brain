/**
 * Always-allow rule store for tool permissions (Inc 17). Rules persist as
 * the `permissions.allow` array of the user's brain config file — the same
 * document themeStore owns a key of — and every check reads fresh from disk
 * so edits (via /permissions or by hand) apply immediately.
 */
import * as fs from 'fs';
import * as path from 'path';
import { configPath } from './themeStore.js';

export interface AllowRule {
  /** Tool name exactly as the daemon reports it (e.g. 'bash'). */
  tool: string;
  /** Byte prefix matched against the tool's primary input string;
   * '' matches every invocation of the tool. */
  inputPrefix: string;
}

/** Keys searched first, mirroring the dialog summarizer's preference order. */
const PRIMARY_KEYS: readonly string[] = [
  'command',
  'file_path',
  'path',
  'query',
  'pattern',
  'url',
  'prompt',
];

/**
 * The input's primary string: first non-empty trimmed value among the
 * canonical keys, else the first non-empty trimmed string value, else ''.
 * Shared single source of truth for rule matching and dialog display.
 */
export function primaryInputString(input: Record<string, unknown>): string {
  for (const key of PRIMARY_KEYS) {
    const v = input[key];
    if (typeof v === 'string' && v.trim().length > 0) return v.trim();
  }
  for (const v of Object.values(input)) {
    if (typeof v === 'string' && v.trim().length > 0) return v.trim();
  }
  return '';
}

/** First matching rule index, or -1. Byte-exact, case-sensitive prefix. */
export function matchingRuleIndex(
  toolName: string,
  input: Record<string, unknown>,
  rules: readonly AllowRule[],
): number {
  const primary = primaryInputString(input);
  return rules.findIndex((r) => r.tool === toolName && primary.startsWith(r.inputPrefix));
}

function readDoc(): Record<string, unknown> {
  try {
    const parsed = JSON.parse(fs.readFileSync(configPath(), 'utf8')) as unknown;
    return parsed && typeof parsed === 'object' ? (parsed as Record<string, unknown>) : {};
  } catch {
    // missing file / bad JSON / unreadable path -> fresh document
    return {};
  }
}

function writeDoc(doc: Record<string, unknown>): void {
  fs.mkdirSync(path.dirname(configPath()), { recursive: true });
  fs.writeFileSync(configPath(), JSON.stringify(doc, null, 2) + '\n');
}

/** Tolerant parse; anything without a non-empty string `tool` and a string
 * `inputPrefix` is dropped rather than trusted. */
function parseRules(value: unknown): AllowRule[] {
  if (!Array.isArray(value)) return [];
  return value.filter(
    (r): r is AllowRule =>
      r !== null &&
      typeof r === 'object' &&
      typeof (r as AllowRule).tool === 'string' &&
      (r as AllowRule).tool.length > 0 &&
      typeof (r as AllowRule).inputPrefix === 'string',
  );
}

function currentRules(doc: Record<string, unknown>): {
  perms: Record<string, unknown>;
  rules: AllowRule[];
} {
  const raw = doc.permissions;
  const perms =
    raw !== null && typeof raw === 'object' ? (raw as Record<string, unknown>) : {};
  return { perms, rules: parseRules(perms.allow) };
}

/** Tolerant read; missing file/key or malformed entries yield fewer/no rules. */
export function readAllowRules(): AllowRule[] {
  return currentRules(readDoc()).rules;
}

/** Merge-write one rule; an identical existing rule is left as-is. */
export function addAllowRule(rule: AllowRule): void {
  const doc = readDoc();
  const { perms, rules } = currentRules(doc);
  if (!rules.some((r) => r.tool === rule.tool && r.inputPrefix === rule.inputPrefix)) {
    rules.push(rule);
  }
  perms.allow = rules;
  doc.permissions = perms;
  writeDoc(doc);
}

/** Remove the nth rule of the current read order; false when out of range. */
export function removeAllowRule(index: number): boolean {
  const doc = readDoc();
  const { perms, rules } = currentRules(doc);
  if (!Number.isInteger(index) || index < 0 || index >= rules.length) return false;
  rules.splice(index, 1);
  perms.allow = rules;
  doc.permissions = perms;
  writeDoc(doc);
  return true;
}

/** Human description shared by the /permissions listing and removal notes. */
export function describeRule(rule: AllowRule): string {
  return rule.inputPrefix.length > 0
    ? `${rule.tool} — commands starting with "${rule.inputPrefix}"`
    : `${rule.tool} — any invocation`;
}

export function describeRules(rules: readonly AllowRule[]): string[] {
  return rules.map((r, i) => ` ${i + 1}. ${describeRule(r)}`);
}

/**
 * Full output of `/permissions [remove <n>]` as one notice block. Performs
 * its own store reads/writes so the AppShell dispatch stays a single call.
 */
export function runPermissionsCommand(args: readonly string[]): string {
  if (args.length === 0) {
    const rules = readAllowRules();
    if (rules.length === 0) return 'No always-allow rules saved.';
    return [
      `Always-allow rules (${configPath()}):`,
      ...describeRules(rules),
      'Remove with: /permissions remove <n>',
    ].join('\n');
  }
  if (args[0] === 'remove') {
    const raw = args[1] ?? '';
    if (!/^\d+$/.test(raw)) return 'Usage: /permissions remove <rule number>';
    const n = Number.parseInt(raw, 10);
    const target = readAllowRules()[n - 1];
    if (target === undefined) return `No rule ${n}.`;
    removeAllowRule(n - 1);
    return `Removed rule ${n} (${describeRule(target)}).`;
  }
  return 'Usage: /permissions [remove <rule number>]';
}
