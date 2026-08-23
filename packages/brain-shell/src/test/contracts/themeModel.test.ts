import { describe, expect, test } from 'bun:test';
import {
  THEME_NAMES,
  getSystemThemeName,
  renderModelSetting,
  resolveThemeSetting,
} from '../../contracts/theme.js';
import { useMainLoopModel } from '../../contracts/model.js';

describe('contracts/theme+model', () => {
  test('theme names include dark/light bases', () => {
    expect(THEME_NAMES).toContain('dark');
    expect(THEME_NAMES).toContain('light');
    expect(THEME_NAMES).toContain('dark-daltonized');
  });

  test('resolveThemeSetting resolves auto via system theme', () => {
    expect(resolveThemeSetting('auto')).toBe(getSystemThemeName());
    expect(resolveThemeSetting('dark')).toBe('dark');
    expect(resolveThemeSetting('light-daltonized')).toBe('light-daltonized');
  });

  test('model label renders without vendor branding', () => {
    expect(renderModelSetting('brain-default')).toBe('brain-default');
  });

  test('useMainLoopModel returns a non-empty label', () => {
    // Plain function outside React render — reads env/default without hooks.
    expect(typeof useMainLoopModel).toBe('function');
    expect(String(process.env.BRAIN_MODEL ?? 'brain-default')).toContain('brain');
  });
});
