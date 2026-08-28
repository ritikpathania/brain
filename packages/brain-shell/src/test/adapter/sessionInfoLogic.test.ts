import { describe, expect, it } from 'bun:test';
import {
  formatSessionInfoSummary,
  SessionController,
} from '../../state/sessionController.js';
import { MockBrainBackendClient } from '../../client/BrainBackendClient.js';

describe('Increment 46: Active Session Composite Info Inspector', () => {
  describe('formatSessionInfoSummary helper', () => {
    it('formats default ephemeral unstarted session correctly', () => {
      const formatted = formatSessionInfoSummary({
        turnCount: 0,
        modelName: 'brain-default',
        thinkingBudget: null,
        systemPrompt: null,
      });

      expect(formatted).toContain('Session ID: ephemeral (unstarted)');
      expect(formatted).toContain('Transcript Turns: 0');
      expect(formatted).toContain('Active Model: brain-default');
      expect(formatted).toContain('Reasoning Budget: disabled (default)');
      expect(formatted).toContain('System Instructions: default engine prompt (no custom instructions)');
      expect(formatted).toContain('Latest Context: none (no retrieval turns executed yet)');
    });

    it('formats active session with reasoning model and custom instructions', () => {
      const formatted = formatSessionInfoSummary({
        sessionId: 'sess-abc-123',
        turnCount: 4,
        modelName: 'claude-3-7-sonnet-20250219',
        supportsThinking: true,
        thinkingBudget: 4096,
        systemPrompt: 'You are an expert Rust systems architect.',
        latestTelemetry: {
          count: 3,
          tokenCount: 245,
          latencyMs: 14,
        },
      });

      expect(formatted).toContain('Session ID: sess-abc-123');
      expect(formatted).toContain('Transcript Turns: 4');
      expect(formatted).toContain('Active Model: claude-3-7-sonnet-20250219');
      expect(formatted).toContain('Reasoning Budget: 4096 tokens (active)');
      expect(formatted).toContain('System Instructions: “You are an expert Rust systems architect.”');
      expect(formatted).toContain('Latest Context: 3 memories (245 tokens, 14ms)');
    });

    it('flags reasoning budget as inactive when active model lacks thinking support', () => {
      const formatted = formatSessionInfoSummary({
        sessionId: 'sess-qwen-456',
        turnCount: 1,
        modelName: 'qwen2.5-coder:32b',
        supportsThinking: false,
        thinkingBudget: 2048,
        systemPrompt: null,
      });

      expect(formatted).toContain('Active Model: qwen2.5-coder:32b');
      expect(formatted).toContain(
        'Reasoning Budget: 2048 tokens (inactive: unsupported by active model)',
      );
    });

    it('truncates long system prompt in summary preview', () => {
      const longPrompt = 'A'.repeat(120);
      const formatted = formatSessionInfoSummary({
        turnCount: 0,
        modelName: 'brain-default',
        thinkingBudget: null,
        systemPrompt: longPrompt,
      });

      expect(formatted).toContain('System Instructions: “' + 'A'.repeat(77) + '...”');
    });
  });

  describe('SessionController.reportSessionInfo() integration', () => {
    it('emits a notice containing active session summary without mutating state', () => {
      const mockClient = new MockBrainBackendClient();
      const controller = new SessionController(mockClient);

      controller.setSystemPrompt('Analyze performance profiles');
      controller.setThinkingBudget(1024);

      // Report session info
      controller.reportSessionInfo();

      const snapshot = controller.getSnapshot();
      const systemRows = snapshot.rows.filter((r) => r.kind === 'system');
      expect(systemRows.length).toBeGreaterThanOrEqual(1);

      const latestNotice = systemRows[systemRows.length - 1];
      expect(latestNotice.text).toContain('Active Model: brain-default');
      expect(latestNotice.text).toContain('System Instructions: “Analyze performance profiles”');
      expect(latestNotice.text).toContain('Transcript Turns: 0');
    });
  });
});
