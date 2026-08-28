/**
 * Increment 47: Unit tests for retrieval context budget steerability (/context budget).
 */
import { describe, it, expect, mock } from 'bun:test';
import {
  SessionController,
  formatSessionInfoSummary,
} from '../../state/sessionController.js';
import { getCommand } from '../../commands/registry.js';
import '../../commands/builtin.js';
import type { BrainBackendClient } from '../../client/BrainBackendClient.js';

function createMockClient(): BrainBackendClient {
  return {
    streamText: mock(async function* () {
      yield { type: 'token', text: 'Hello' };
      yield { type: 'finished', status: 'completed' };
    }),
    createSession: mock(async () => ({ sessionId: 'sess-inc47-test' })),
    listSessions: mock(async () => []),
    resumeSession: mock(async () => ({
      sessionId: 'sess-inc47-test',
      title: 'Session Test',
      messages: [],
    })),
    deleteSession: mock(async () => ({ success: true, sessionId: 'sess-inc47-test' })),
    pinSession: mock(async () => ({ success: true, sessionId: 'sess-inc47-test', pinned: true })),
    unpinSession: mock(async () => ({ success: true, sessionId: 'sess-inc47-test', pinned: false })),
    archiveSession: mock(async () => ({
      success: true,
      sessionId: 'sess-inc47-test',
      archived: true,
    })),
    restoreSession: mock(async () => ({
      success: true,
      sessionId: 'sess-inc47-test',
      archived: false,
    })),
    forkSession: mock(async () => ({ parentSessionId: 'p', forkedSessionId: 'f' })),
    renameSession: mock(async () => ({ success: true, sessionId: 's', title: 't' })),
    searchTranscripts: mock(async () => []),
    listModels: mock(async () => ({
      models: [{ id: 'brain-default', name: 'Brain Default', isDefault: true, supportsThinking: false }],
      defaultModelId: 'brain-default',
    })),
    searchMemory: mock(async () => ({ memories: [] })),
    storeMemory: mock(async () => ({ success: true, nodeId: 'n1' })),
    deleteMemory: mock(async () => ({ success: true, nodeId: 'n1' })),
    linkMemories: mock(async () => ({ success: true, sourceId: 's', targetId: 't', relation: 'relates_to' })),
    unlinkMemories: mock(async () => ({ success: true, sourceId: 's', targetId: 't', relation: 'relates_to' })),
    consolidateMemories: mock(async () => ({ actionsTaken: 0, durationMs: 5, errors: [] })),
    retrieveContext: mock(async () => ({
      epoch_id: 'ep1',
      memories: [],
      provenance: { count: 0, sources: [], channels: [] },
      token_count: 0,
      serialized_context: '',
    })),
    fetchTurnEvents: mock(async () => []),
    fetchTurnPlan: mock(async () => null),
    grantToolPermission: mock(async () => ({ granted: true })),
    getHealth: mock(async () => ({ status: 'healthy', version: '1.0.0' })),
    getSystemStatus: mock(async () => ({
      status: 'healthy',
      version: '1.0.0',
      uptime_seconds: 100,
      storage: { healthy: true, sessions_count: 1, nodes_count: 0, edges_count: 0 },
      models: { count: 1, active_model: 'brain-default' },
    })),
    onConnectionChange: mock(() => () => {}),
  } as unknown as BrainBackendClient;
}

describe('Increment 47: Retrieval Context Budget Steerability', () => {
  describe('SessionController.setContextBudget & validation', () => {
    it('accepts valid positive integers and emits confirmation', () => {
      const client = createMockClient();
      const controller = new SessionController(client);

      expect(controller.currentContextMaxTokens).toBeNull();
      const ok = controller.setContextBudget(4096);
      expect(ok).toBe(true);
      expect(controller.currentContextMaxTokens).toBe(4096);

      const rows = controller.getSnapshot().rows;
      const lastRow = rows[rows.length - 1];
      expect(lastRow.text).toContain('Context retrieval budget set to 4096 tokens.');
    });

    it('rejects zero, negative numbers, floats, and NaN without mutating state', () => {
      const client = createMockClient();
      const controller = new SessionController(client);
      controller.setContextBudget(4096);

      // zero
      expect(controller.setContextBudget(0)).toBe(false);
      expect(controller.currentContextMaxTokens).toBe(4096);

      // negative
      expect(controller.setContextBudget(-100)).toBe(false);
      expect(controller.currentContextMaxTokens).toBe(4096);

      // float
      expect(controller.setContextBudget(1024.5)).toBe(false);
      expect(controller.currentContextMaxTokens).toBe(4096);

      // NaN
      expect(controller.setContextBudget(NaN)).toBe(false);
      expect(controller.currentContextMaxTokens).toBe(4096);

      // non-finite
      expect(controller.setContextBudget(Infinity)).toBe(false);
      expect(controller.currentContextMaxTokens).toBe(4096);
    });

    it('resets to null when budget is null (engine default)', () => {
      const client = createMockClient();
      const controller = new SessionController(client);
      controller.setContextBudget(4096);
      expect(controller.currentContextMaxTokens).toBe(4096);

      const ok = controller.setContextBudget(null);
      expect(ok).toBe(true);
      expect(controller.currentContextMaxTokens).toBeNull();

      const rows = controller.getSnapshot().rows;
      const lastRow = rows[rows.length - 1];
      expect(lastRow.text).toContain('Context retrieval budget reset to engine default (2048 tokens).');
    });

    it('reports truthful context budget status for both configured and default states', () => {
      const client = createMockClient();
      const controller = new SessionController(client);

      // Default
      controller.reportContextBudgetStatus();
      let rows = controller.getSnapshot().rows;
      expect(rows[rows.length - 1].text).toContain('2048 tokens (engine default)');

      // Configured
      controller.setContextBudget(8192);
      controller.reportContextBudgetStatus();
      rows = controller.getSnapshot().rows;
      expect(rows[rows.length - 1].text).toContain('8192 tokens (configured)');
    });
  });

  describe('formatSessionInfoSummary context budget formatting', () => {
    it('formats default context budget when null or unspecified', () => {
      const formatted = formatSessionInfoSummary({
        turnCount: 0,
        modelName: 'brain-default',
        thinkingBudget: null,
        systemPrompt: null,
        contextMaxTokens: null,
      });
      expect(formatted).toContain('Context Budget: 2048 tokens (engine default)');
    });

    it('formats configured context budget when set', () => {
      const formatted = formatSessionInfoSummary({
        turnCount: 2,
        modelName: 'brain-default',
        thinkingBudget: null,
        systemPrompt: null,
        contextMaxTokens: 4096,
      });
      expect(formatted).toContain('Context Budget: 4096 tokens (configured)');
    });
  });

  describe('Command parsing & dispatch', () => {
    it('/context without args returns overlay action', () => {
      const cmd = getCommand('context');
      expect(cmd).toBeDefined();
      expect(cmd!.run({ args: [] })).toEqual({ type: 'overlay', overlay: 'context' });
    });

    it('/context budget without args returns context-budget-status action', () => {
      const cmd = getCommand('context');
      expect(cmd!.run({ args: ['budget'] })).toEqual({
        type: 'action',
        action: 'context-budget-status',
      });
    });

    it('/context budget 4096 returns context-budget-set action with string number', () => {
      const cmd = getCommand('context');
      expect(cmd!.run({ args: ['budget', '4096'] })).toEqual({
        type: 'action',
        action: 'context-budget-set',
        args: ['4096'],
      });
    });

    it('/context budget default (and aliases reset/none/auto) returns context-budget-set default', () => {
      const cmd = getCommand('context');
      expect(cmd!.run({ args: ['budget', 'default'] })).toEqual({
        type: 'action',
        action: 'context-budget-set',
        args: ['default'],
      });
      expect(cmd!.run({ args: ['budget', 'reset'] })).toEqual({
        type: 'action',
        action: 'context-budget-set',
        args: ['default'],
      });
    });

    it('/context budget with invalid tokens returns usage text', () => {
      const cmd = getCommand('context');
      expect(cmd!.run({ args: ['budget', 'abc'] })).toEqual({
        type: 'text',
        value: 'Usage: /context budget <tokens | default>',
      });
      expect(cmd!.run({ args: ['budget', '-5'] })).toEqual({
        type: 'text',
        value: 'Usage: /context budget <tokens | default>',
      });
    });
  });

  describe('SessionController.submit generation propagation', () => {
    it('passes configured contextMaxTokens into streamText request', async () => {
      const client = createMockClient();
      const controller = new SessionController(client);

      controller.setContextBudget(3000);
      await controller.submit('Test prompt with 3000 tokens budget');

      const streamCalls = (client.streamText as any).mock.calls;
      expect(streamCalls.length).toBe(1);
      const req = streamCalls[0][0];
      expect(req.contextMaxTokens).toBe(3000);
    });

    it('passes undefined contextMaxTokens into streamText request when budget is null', async () => {
      const client = createMockClient();
      const controller = new SessionController(client);

      await controller.submit('Test prompt with default budget');

      const streamCalls = (client.streamText as any).mock.calls;
      expect(streamCalls.length).toBe(1);
      const req = streamCalls[0][0];
      expect(req.contextMaxTokens).toBeUndefined();
    });
  });
});
