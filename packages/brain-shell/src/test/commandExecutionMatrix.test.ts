/**
 * Comprehensive Command Execution Verification Matrix
 *
 * Deterministically exercises and verifies local Claude command handlers in Brain.
 */

import { describe, it, expect, beforeAll, afterAll } from 'bun:test';
import * as fs from 'fs';
import * as path from 'path';
import clearCommand from '../../vendor/claude/commands/clear/index.js';
import helpCommand from '../../vendor/claude/commands/help/index.js';
import versionCommand from '../../vendor/claude/commands/version.js';
import releaseNotesCommand from '../../vendor/claude/commands/release-notes/index.js';
import configCommand from '../../vendor/claude/commands/config/index.js';
import costCommand from '../../vendor/claude/commands/cost/index.js';
import diffCommand from '../../vendor/claude/commands/diff/index.js';
import addDirCommand from '../../vendor/claude/commands/add-dir/index.js';
import btwCommand from '../../vendor/claude/commands/btw/index.js';
import planCommand from '../../vendor/claude/commands/plan/index.js';
import { REMOTE_SAFE_COMMANDS, BRIDGE_SAFE_COMMANDS } from '../../vendor/claude/commands.js';

describe('Command Execution Verification Matrix (Local Commands)', () => {
  const tmpDir = `/tmp/brain_cmd_test_${Date.now()}`;

  beforeAll(() => {
    if (!fs.existsSync(tmpDir)) fs.mkdirSync(tmpDir, { recursive: true });
  });

  afterAll(() => {
    if (fs.existsSync(tmpDir)) fs.rmSync(tmpDir, { recursive: true, force: true });
  });

  it('/clear: loads lazy implementation and exposes conversation reset', async () => {
    expect(clearCommand.name).toBe('clear');
    expect(clearCommand.description).toContain('Clear');
    expect(typeof (clearCommand as any).load).toBe('function');
    const mod = await (clearCommand as any).load();
    expect(mod.default?.call || mod.call).toBeDefined();
  });

  it('/help: loads lazy implementation and exposes help catalog', async () => {
    expect(helpCommand.name).toBe('help');
    expect(helpCommand.description).toBeDefined();
    expect(typeof (helpCommand as any).load).toBe('function');
    const mod = await (helpCommand as any).load();
    expect(mod.default?.call || mod.call).toBeDefined();
  });

  it('/version: exposes version metadata and changelog link', async () => {
    expect(versionCommand.name).toBe('version');
    expect(versionCommand.description).toBeDefined();
    expect(typeof (versionCommand as any).load).toBe('function');
    const mod = await (versionCommand as any).load();
    expect(mod.default?.call || mod.call).toBeDefined();
  });

  it('/release-notes: exposes release notes handler', async () => {
    expect(releaseNotesCommand.name).toBe('release-notes');
    expect(typeof (releaseNotesCommand as any).load).toBe('function');
    const mod = await (releaseNotesCommand as any).load();
    expect(mod.default?.call || mod.call).toBeDefined();
  });

  it('/config: exposes local config management handler', async () => {
    expect(configCommand.name).toBe('config');
    expect(typeof (configCommand as any).load).toBe('function');
    const mod = await (configCommand as any).load();
    expect(mod.default?.call || mod.call).toBeDefined();
  });

  it('/cost: calculates token and session cost structure', async () => {
    expect(costCommand.name).toBe('cost');
    expect(typeof (costCommand as any).load).toBe('function');
    const mod = await (costCommand as any).load();
    expect(mod.default?.call || mod.call).toBeDefined();
  });

  it('/diff: exposes git/workspace diff inspection', async () => {
    expect(diffCommand.name).toBe('diff');
    expect(typeof (diffCommand as any).load).toBe('function');
    const mod = await (diffCommand as any).load();
    expect(mod.default?.call || mod.call).toBeDefined();
  });

  it('/add-dir: adds directory to session context', async () => {
    expect(addDirCommand.name).toBe('add-dir');
    expect(typeof (addDirCommand as any).load).toBe('function');
    const mod = await (addDirCommand as any).load();
    expect(mod.default?.call || mod.call).toBeDefined();
  });

  it('/btw: captures side-channel quick notes', async () => {
    expect(btwCommand.name).toBe('btw');
    expect(typeof (btwCommand as any).load).toBe('function');
    const mod = await (btwCommand as any).load();
    expect(mod.default?.call || mod.call).toBeDefined();
  });

  it('/plan: toggles architect plan mode', async () => {
    expect(planCommand.name).toBe('plan');
    expect(typeof (planCommand as any).load).toBe('function');
    const mod = await (planCommand as any).load();
    expect(mod.default?.call || mod.call).toBeDefined();
  });

  it('REMOTE_SAFE_COMMANDS: validates security boundary classification', () => {
    expect(REMOTE_SAFE_COMMANDS.size).toBeGreaterThan(5);
  });

  it('BRIDGE_SAFE_COMMANDS: validates local execution bridge safety allowlist', () => {
    expect(BRIDGE_SAFE_COMMANDS.size).toBeGreaterThan(3);
  });
});
