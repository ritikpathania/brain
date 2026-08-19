import { describe, test, expect } from 'bun:test';

process.env.ANTHROPIC_API_KEY = process.env.ANTHROPIC_API_KEY || 'test-key';
import * as os from 'os';
import * as path from 'path';
import * as fs from 'fs';
import { getGlobalConfig, saveGlobalConfig } from '../../vendor/claude/utils/config.js';
import { getCommands, getCommand } from '../../vendor/claude/commands.js';
import { generateCommandSuggestions, applyCommandSuggestion } from '../../vendor/claude/utils/suggestions/commandSuggestions.js';
import themeCommand from '../../vendor/claude/commands/theme/index.js';
import vimCommand from '../../vendor/claude/commands/vim/index.js';

describe('Theme and Vim Command Integration', () => {
  test('theme command exists and is local-jsx', async () => {
    const commands = await getCommands(process.cwd());
    const theme = commands.find(c => c.name === 'theme');
    expect(theme).toBeDefined();
    expect(theme?.type).toBe('local-jsx');
    expect(theme?.description).toBe('Change the theme');
  });

  test('generateCommandSuggestions finds /theme on exact and partial match', async () => {
    const commands = await getCommands(process.cwd());
    const suggestionsPartial = generateCommandSuggestions('/the', commands);
    expect(suggestionsPartial.some(s => s.displayText === '/theme')).toBe(true);

    const suggestionsExact = generateCommandSuggestions('/theme', commands);
    expect(suggestionsExact.length).toBeGreaterThan(0);
    expect(suggestionsExact[0].displayText).toBe('/theme');
  });

  test('applyCommandSuggestion formats /theme and submits as slash command', async () => {
    const commands = await getCommands(process.cwd());
    const suggestions = generateCommandSuggestions('/theme', commands);
    expect(suggestions.length).toBeGreaterThan(0);

    let submittedValue = '';
    let isSlash = false;
    let inputVal = '';
    let cursor = 0;

    applyCommandSuggestion(
      suggestions[0],
      true,
      commands,
      (v) => { inputVal = v; },
      (c) => { cursor = c; },
      (v, slash) => {
        submittedValue = v;
        isSlash = !!slash;
      }
    );

    expect(inputVal).toBe('/theme ');
    expect(cursor).toBe(7);
    expect(submittedValue).toBe('/theme ');
    expect(isSlash).toBe(true);
  });

  test('vim command toggles editorMode between normal and vim', async () => {
    const commands = await getCommands(process.cwd());
    const vim = commands.find(c => c.name === 'vim');
    expect(vim).toBeDefined();
    expect(vim?.type).toBe('local');

    const initialMode = getGlobalConfig().editorMode || 'normal';
    
    // Toggle vim mode
    const mod = await vim?.load();
    const result = await mod.call('vim', {} as any);
    const newConfig = getGlobalConfig();
    
    if (initialMode === 'vim') {
      expect(newConfig.editorMode).toBe('normal');
    } else {
      expect(newConfig.editorMode).toBe('vim');
    }

    // Toggle back
    await mod.call('vim', {} as any);
    expect(getGlobalConfig().editorMode).toBe(initialMode);
  });

  test('theme module loads ThemePicker correctly', async () => {
    const mod = await themeCommand.load();
    expect(mod.call).toBeDefined();
    
    let doneCalled = false;
    let doneResult: string | undefined;
    
    const onDone = (res?: string) => {
      doneCalled = true;
      doneResult = res;
    };
    
    const element = await mod.call(onDone, {} as any);
    expect(element).toBeDefined();
  });
});
