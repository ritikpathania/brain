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
  public shouldFail = false;

  async createSession(): Promise<any> {
    return { sessionId: 'sess-sysprompt-test', title: 'Test Session', createdAtMs: Date.now() };
  }

  async *streamText(request: BrainGenerationRequest): AsyncIterable<BrainStreamChunk> {
    this.lastRequest = request;
    if (this.shouldFail) {
      throw new Error('Simulated backend stream failure');
    }
    yield {
      type: 'token',
      token: 'Hello with custom instructions',
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

describe('Increment 44: SessionController System Prompt & Instructions Logic', () => {
  it('defaults to null and reports default prompt status', () => {
    const mock = new MockBackendClient();
    const controller = new SessionController(mock as unknown as BrainBackendClient);

    expect(controller.currentSystemPrompt).toBeNull();

    controller.reportSystemPromptStatus();
    const lastRow = controller.getSnapshot().rows.slice(-1)[0];
    expect((lastRow as any)?.text).toContain(
      'Default engine prompt active (no custom instructions configured).',
    );
  });

  it('sets custom instructions and formats confirmation notice', () => {
    const mock = new MockBackendClient();
    const controller = new SessionController(mock as unknown as BrainBackendClient);

    const promptText = 'You are an expert Rust systems architect focusing on zero-allocation code.';
    const ok = controller.setSystemPrompt(promptText);

    expect(ok).toBe(true);
    expect(controller.currentSystemPrompt).toBe(promptText);

    const lastRow = controller.getSnapshot().rows.slice(-1)[0];
    expect((lastRow as any)?.text).toContain('Set system instructions:');
    expect((lastRow as any)?.text).toContain('You are an expert Rust systems architect');

    controller.reportSystemPromptStatus();
    const statusRow = controller.getSnapshot().rows.slice(-1)[0];
    expect((statusRow as any)?.text).toContain(`Active system instructions: “${promptText}”`);
  });

  it('clears custom instructions on null or keywords (clear, off, reset, none, disable)', () => {
    const mock = new MockBackendClient();
    const controller = new SessionController(mock as unknown as BrainBackendClient);

    const keywords = [null, 'clear', 'off', 'reset', 'none', 'disable', 'CLEAR', '  Off  '];

    for (const kw of keywords) {
      controller.setSystemPrompt('Custom instruction prompt');
      expect(controller.currentSystemPrompt).toBe('Custom instruction prompt');

      const ok = controller.setSystemPrompt(kw);
      expect(ok).toBe(true);
      expect(controller.currentSystemPrompt).toBeNull();

      const lastRow = controller.getSnapshot().rows.slice(-1)[0];
      expect((lastRow as any)?.text).toContain(
        'Cleared custom system instructions (using default engine prompt).',
      );
    }
  });

  it('rejects whitespace-only prompts without mutating previous state', () => {
    const mock = new MockBackendClient();
    const controller = new SessionController(mock as unknown as BrainBackendClient);

    controller.setSystemPrompt('Initial prompt');
    expect(controller.currentSystemPrompt).toBe('Initial prompt');

    const invalidInputs = ['', '   ', '\t\n '];
    for (const invalid of invalidInputs) {
      const ok = controller.setSystemPrompt(invalid);
      expect(ok).toBe(false);
      expect(controller.currentSystemPrompt).toBe('Initial prompt');

      const lastRow = controller.getSnapshot().rows.slice(-1)[0];
      expect((lastRow as any)?.text).toContain(
        'System instructions cannot be empty or whitespace only.',
      );
    }
  });

  it('submits turn with systemPrompt populated when configured', async () => {
    const mock = new MockBackendClient();
    const controller = new SessionController(mock as unknown as BrainBackendClient);

    const instructions = 'You are a helpful TypeScript and Rust compiler assistant.';
    controller.setSystemPrompt(instructions);

    await controller.submit('Explain borrow checker');

    expect(mock.lastRequest).toBeDefined();
    expect(mock.lastRequest?.systemPrompt).toBe(instructions);
  });

  it('submits turn with systemPrompt as undefined (never empty string or null) when unset or cleared', async () => {
    const mock = new MockBackendClient();
    const controller = new SessionController(mock as unknown as BrainBackendClient);

    // 1. Initial unset state
    await controller.submit('Prompt 1');
    expect(mock.lastRequest).toBeDefined();
    expect(mock.lastRequest?.systemPrompt).toBeUndefined();

    // 2. Set then clear
    controller.setSystemPrompt('Temporary prompt');
    controller.setSystemPrompt('clear');

    await controller.submit('Prompt 2');
    expect(mock.lastRequest).toBeDefined();
    expect(mock.lastRequest?.systemPrompt).toBeUndefined();
  });

  it('preserves configured system prompt state even if generation stream fails', async () => {
    const mock = new MockBackendClient();
    mock.shouldFail = true;
    const controller = new SessionController(mock as unknown as BrainBackendClient);

    controller.setSystemPrompt('Persistent instruction');

    await controller.submit('Failing request');

    expect(controller.currentSystemPrompt).toBe('Persistent instruction');
    expect(mock.lastRequest?.systemPrompt).toBe('Persistent instruction');
  });

  describe('Command Surface: /system and /instructions', () => {
    it('parses /system with no arguments to system-status', () => {
      const cmd = getCommand('system');
      expect(cmd).toBeDefined();

      expect(cmd!.run({ args: [] })).toEqual({
        type: 'action',
        action: 'system-status',
      });

      expect(cmd!.run({ args: ['   '] })).toEqual({
        type: 'action',
        action: 'system-status',
      });
    });

    it('parses /system with arguments to system-set', () => {
      const cmd = getCommand('system');
      expect(cmd).toBeDefined();

      const res = cmd!.run({ args: ['You', 'are', 'a', 'concise', 'bot'] });
      expect(res).toEqual({
        type: 'action',
        action: 'system-set',
        args: ['You are a concise bot'],
      });
    });

    it('aliases /instructions and /sysprompt to identical command behavior', () => {
      const instructionsCmd = getCommand('instructions');
      const syspromptCmd = getCommand('sysprompt');

      expect(instructionsCmd).toBeDefined();
      expect(syspromptCmd).toBeDefined();

      expect(instructionsCmd!.run({ args: [] })).toEqual({
        type: 'action',
        action: 'system-status',
      });

      expect(syspromptCmd!.run({ args: ['clear'] })).toEqual({
        type: 'action',
        action: 'system-set',
        args: ['clear'],
      });
    });
  });
});
