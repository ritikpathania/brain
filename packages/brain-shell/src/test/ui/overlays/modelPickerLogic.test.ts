/**
 * Inc 38: Unit tests for pure model picker VM transformation and keyboard navigation.
 */
import { describe, it, expect } from 'bun:test';
import {
  formatContextTokens,
  toModelItemVM,
  clampModelSelection,
  modelListDecision,
} from '../../../ui/overlays/modelPickerLogic.js';
import type { BrainModelDescriptor } from '../../../client/BrainBackendClient.js';

describe('modelPickerLogic', () => {
  const FIXTURES: BrainModelDescriptor[] = [
    {
      id: 'brain-default',
      name: 'Brain Default',
      provider: 'local',
      contextWindow: 128000,
      maxOutputTokens: 8192,
      supportsThinking: true,
      supportsTools: true,
      isDefault: true,
    },
    {
      id: 'claude-3-5-sonnet',
      name: 'Claude 3.5 Sonnet',
      provider: 'anthropic',
      contextWindow: 200000,
      maxOutputTokens: 8192,
      supportsThinking: true,
      supportsTools: true,
      isDefault: false,
    },
  ];

  it('formatContextTokens formats thousands to k', () => {
    expect(formatContextTokens(128000)).toBe('128k ctx');
    expect(formatContextTokens(200000)).toBe('200k ctx');
    expect(formatContextTokens(800)).toBe('800 ctx');
  });

  it('toModelItemVM marks default as active when activeModelId is not specified', () => {
    const vms = toModelItemVM(FIXTURES);
    expect(vms).toHaveLength(2);
    expect(vms[0]!.isActive).toBe(true);
    expect(vms[0]!.isDefault).toBe(true);
    expect(vms[0]!.contextLabel).toBe('128k ctx');
    expect(vms[1]!.isActive).toBe(false);
  });

  it('toModelItemVM marks explicit activeModelId correctly', () => {
    const vms = toModelItemVM(FIXTURES, 'claude-3-5-sonnet');
    expect(vms[0]!.isActive).toBe(false);
    expect(vms[1]!.isActive).toBe(true);
  });

  it('clampModelSelection clamps into valid range', () => {
    expect(clampModelSelection(-1, 3)).toBe(0);
    expect(clampModelSelection(1, 3)).toBe(1);
    expect(clampModelSelection(5, 3)).toBe(2);
    expect(clampModelSelection(0, 0)).toBe(0);
  });

  it('modelListDecision navigates, commits, and cancels', () => {
    expect(modelListDecision('up', 1, 3)).toEqual({ nextSelected: 0 });
    expect(modelListDecision('up', 0, 3)).toEqual({ nextSelected: 0 });
    expect(modelListDecision('down', 1, 3)).toEqual({ nextSelected: 2 });
    expect(modelListDecision('down', 2, 3)).toEqual({ nextSelected: 2 });
    expect(modelListDecision('commit', 1, 3)).toEqual({ nextSelected: 1, committed: true });
    expect(modelListDecision('cancel', 1, 3)).toEqual({ nextSelected: 1, cancelled: true });
    expect(modelListDecision('other', 1, 3)).toEqual({ nextSelected: 1 });
  });
});
