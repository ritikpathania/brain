import { describe, expect, it } from 'bun:test';
import { formatThinkingIndicator } from '../../../ui/shell/StatusBar.js';
import { SessionController } from '../../../state/sessionController.js';
import type {
  BrainBackendClient,
  BrainModelDescriptor,
} from '../../../client/BrainBackendClient.js';

describe('formatThinkingIndicator helper', () => {
  it('formats think:n/a when supportsThinking is false', () => {
    expect(formatThinkingIndicator(false, null)).toBe('think:n/a');
    expect(formatThinkingIndicator(false, 4096)).toBe('think:n/a');
    expect(formatThinkingIndicator(false, 8192)).toBe('think:n/a');
  });

  it('formats think:off when budget is null, undefined, or 0', () => {
    expect(formatThinkingIndicator(true, null)).toBe('think:off');
    expect(formatThinkingIndicator(true, undefined)).toBe('think:off');
    expect(formatThinkingIndicator(true, 0)).toBe('think:off');
    expect(formatThinkingIndicator(undefined, null)).toBe('think:off');
  });

  it('formats thousands into k notation (e.g. think:4k, think:8k)', () => {
    expect(formatThinkingIndicator(true, 4096)).toBe('think:4k');
    expect(formatThinkingIndicator(true, 8192)).toBe('think:8k');
    expect(formatThinkingIndicator(true, 16384)).toBe('think:16k');
    expect(formatThinkingIndicator(undefined, 2048)).toBe('think:2k');
  });

  it('formats sub-thousand budgets numerically (e.g. think:512)', () => {
    expect(formatThinkingIndicator(true, 512)).toBe('think:512');
    expect(formatThinkingIndicator(true, 800)).toBe('think:800');
  });
});

describe('SessionController capability awareness & status reflection', () => {
  const defaultModel: BrainModelDescriptor = {
    id: 'brain-default',
    name: 'Brain Default',
    provider: 'mock',
    contextWindow: 128000,
    maxOutputTokens: 8192,
    supportsThinking: true,
    supportsTools: true,
    isDefault: true,
  };

  const nonThinkingModel: BrainModelDescriptor = {
    id: 'qwen2.5-coder:32b',
    name: 'Qwen 2.5 Coder',
    provider: 'mock',
    contextWindow: 128000,
    maxOutputTokens: 8192,
    supportsThinking: false,
    supportsTools: true,
    isDefault: false,
  };

  class MockClient implements Partial<BrainBackendClient> {
    async resolveModel(query?: string): Promise<BrainModelDescriptor> {
      if (query?.includes('qwen')) return nonThinkingModel;
      return defaultModel;
    }
  }

  it('emits non-thinking notice when setting budget on a non-thinking model', async () => {
    const mock = new MockClient();
    const controller = new SessionController(mock as unknown as BrainBackendClient);

    await controller.switchModel('qwen');
    expect(controller.getSnapshot().activeModel?.supportsThinking).toBe(false);

    controller.setThinkingBudget(4096);
    expect(controller.currentThinkingBudget).toBe(4096);
    expect(controller.getSnapshot().thinkingBudget).toBe(4096);

    const rows = controller.getSnapshot().rows;
    const warningRow = rows.find((r) =>
      (r as any).text?.includes('does not support reasoning thinking'),
    );
    expect(warningRow).toBeDefined();
  });

  it('preserves configured budget across non-thinking and thinking model switches', async () => {
    const mock = new MockClient();
    const controller = new SessionController(mock as unknown as BrainBackendClient);

    controller.setThinkingBudget(4096);
    expect(controller.currentThinkingBudget).toBe(4096);

    // Switch to non-thinking
    await controller.switchModel('qwen');
    expect(controller.currentThinkingBudget).toBe(4096); // preference preserved
    expect(controller.getSnapshot().activeModel?.supportsThinking).toBe(false);

    // Switch back to thinking
    await controller.switchModel('brain-default');
    expect(controller.currentThinkingBudget).toBe(4096);
    expect(controller.getSnapshot().activeModel?.supportsThinking).toBe(true);
  });
});
