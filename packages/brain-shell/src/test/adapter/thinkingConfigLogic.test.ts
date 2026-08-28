import { describe, expect, it } from 'bun:test';
import { SessionController } from '../../state/sessionController.js';
import { getCommand } from '../../commands/registry.js';
import '../../commands/builtin.js';
import type {
  BrainBackendClient,
  BrainGenerationRequest,
  BrainStreamChunk,
} from '../../client/BrainBackendClient.js';

class MockBackendClient implements Partial<BrainBackendClient> {
  public lastRequest?: BrainGenerationRequest;

  async createSession(): Promise<any> {
    return { sessionId: 'sess-think-test', title: 'Test Session', createdAtMs: Date.now() };
  }

  async *streamText(request: BrainGenerationRequest): AsyncIterable<BrainStreamChunk> {
    this.lastRequest = request;
    yield {
      type: 'token',
      token: 'Hello with thinking',
      sequence: 1,
    };
    yield {
      type: 'finished',
      status: 'completed',
      sequence: 2,
    };
  }

  onChunk(): () => void {
    return () => {};
  }
}

describe('SessionController thinking budget state & transitions', () => {
  it('defaults to disabled (null)', () => {
    const mock = new MockBackendClient();
    const controller = new SessionController(mock as unknown as BrainBackendClient);

    expect(controller.currentThinkingBudget).toBeNull();
    expect(controller.getSnapshot().thinkingBudget).toBeNull();

    controller.reportThinkingStatus();
    const lastRow = controller.getSnapshot().rows.slice(-1)[0];
    expect((lastRow as any)?.text).toContain('Reasoning thinking budget is disabled (default).');
  });

  it('sets positive integer budget and updates snapshot and notice', () => {
    const mock = new MockBackendClient();
    const controller = new SessionController(mock as unknown as BrainBackendClient);

    const ok = controller.setThinkingBudget(4096);
    expect(ok).toBe(true);
    expect(controller.currentThinkingBudget).toBe(4096);
    expect(controller.getSnapshot().thinkingBudget).toBe(4096);

    const lastRow = controller.getSnapshot().rows.slice(-1)[0];
    expect((lastRow as any)?.text).toContain('Reasoning thinking budget set to 4096 tokens.');

    controller.reportThinkingStatus();
    const statusRow = controller.getSnapshot().rows.slice(-1)[0];
    expect((statusRow as any)?.text).toContain('Current reasoning thinking budget: 4096 tokens.');
  });

  it('disables budget on setThinkingBudget(null)', () => {
    const mock = new MockBackendClient();
    const controller = new SessionController(mock as unknown as BrainBackendClient);

    controller.setThinkingBudget(8192);
    expect(controller.currentThinkingBudget).toBe(8192);

    const ok = controller.setThinkingBudget(null);
    expect(ok).toBe(true);
    expect(controller.currentThinkingBudget).toBeNull();
    expect(controller.getSnapshot().thinkingBudget).toBeNull();

    const lastRow = controller.getSnapshot().rows.slice(-1)[0];
    expect((lastRow as any)?.text).toContain('Reasoning thinking budget disabled.');
  });

  it('atomically rejects non-integer, negative, zero, or NaN budgets without mutating previous state', () => {
    const mock = new MockBackendClient();
    const controller = new SessionController(mock as unknown as BrainBackendClient);

    controller.setThinkingBudget(2048);
    expect(controller.currentThinkingBudget).toBe(2048);

    const invalidInputs = [0, -100, 1024.5, NaN, Infinity, -Infinity];
    for (const invalid of invalidInputs) {
      const ok = controller.setThinkingBudget(invalid);
      expect(ok).toBe(false);
      expect(controller.currentThinkingBudget).toBe(2048);
      expect(controller.getSnapshot().thinkingBudget).toBe(2048);

      const lastRow = controller.getSnapshot().rows.slice(-1)[0];
      expect((lastRow as any)?.text).toContain(
        'Thinking budget must be a positive integer (tokens), or "off" to disable.',
      );
    }
  });

  it('transmits thinkingConfig in submit request when budget is active', async () => {
    const mock = new MockBackendClient();
    const controller = new SessionController(mock as unknown as BrainBackendClient);

    controller.setThinkingBudget(4096);
    await controller.submit('Explain reasoning mechanisms');

    expect(mock.lastRequest).toBeDefined();
    expect(mock.lastRequest?.thinkingConfig).toEqual({
      mode: 'enabled',
      budgetTokens: 4096,
    });
  });

  it('omits thinkingConfig in submit request when budget is disabled', async () => {
    const mock = new MockBackendClient();
    const controller = new SessionController(mock as unknown as BrainBackendClient);

    controller.setThinkingBudget(null);
    await controller.submit('Explain simple fact');

    expect(mock.lastRequest).toBeDefined();
    expect(mock.lastRequest?.thinkingConfig).toBeUndefined();
  });
});

describe('/thinking and /think command routing', () => {
  it('routes bare /thinking and /think to thinking-status action', () => {
    const thinkingCmd = getCommand('thinking')!;
    expect(thinkingCmd).toBeDefined();

    const res1 = thinkingCmd.run({
      args: [],
      raw: '/thinking',
      isBash: false,
    });
    expect(res1).toEqual({ type: 'action', action: 'thinking-status' });

    const thinkCmd = getCommand('think')!;
    expect(thinkCmd).toBeDefined();
    const res2 = thinkCmd.run({
      args: [],
      raw: '/think',
      isBash: false,
    });
    expect(res2).toEqual({ type: 'action', action: 'thinking-status' });
  });

  it('routes /thinking <tokens> to thinking-set action', () => {
    const thinkingCmd = getCommand('thinking')!;
    const res = thinkingCmd.run({
      args: ['4096'],
      raw: '/thinking 4096',
      isBash: false,
    });
    expect(res).toEqual({ type: 'action', action: 'thinking-set', args: ['4096'] });
  });

  it('routes /thinking off / disable to thinking-set off', () => {
    const thinkingCmd = getCommand('thinking')!;

    const res1 = thinkingCmd.run({
      args: ['off'],
      raw: '/thinking off',
      isBash: false,
    });
    expect(res1).toEqual({ type: 'action', action: 'thinking-set', args: ['off'] });

    const res2 = thinkingCmd.run({
      args: ['disable'],
      raw: '/thinking disable',
      isBash: false,
    });
    expect(res2).toEqual({ type: 'action', action: 'thinking-set', args: ['off'] });
  });

  it('returns usage text on invalid tokens', () => {
    const thinkingCmd = getCommand('thinking')!;

    const invalidArgs = ['invalid', '0', '-50', '1024.5', 'abc'];
    for (const arg of invalidArgs) {
      const res = thinkingCmd.run({
        args: [arg],
        raw: `/thinking ${arg}`,
        isBash: false,
      });
      expect(res).toEqual({ type: 'text', value: 'Usage: /thinking [tokens | off]' });
    }
  });
});
