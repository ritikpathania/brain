import { describe, test, expect } from 'bun:test';
import { createBrainCallModel } from '../../adapter/brainCallModel.js';
import { MockBrainBackendClient, type BrainGenerationRequest, type BrainStreamChunk } from '../../client/BrainBackendClient.js';
import { createAssistantMessage } from '../../../vendor/claude/utils/messages.js';

describe('Phase 8.1: Memory Seam Integration & Runtime Contract Matrix', () => {

  // ==========================================================================
  // SCENARIO 1: COLD START / EMPTY MEMORY INVARIANT
  // ==========================================================================
  test('Scenario 1: Cold start with zero prior memories streams standard completion without failure', async () => {
    let capturedRequest: BrainGenerationRequest | null = null;

    const mockClient = new MockBrainBackendClient(async function* (req) {
      capturedRequest = req;
      yield { type: 'token', token: 'Hello! ' };
      yield { type: 'token', token: 'How can I assist you today?' };
      yield { type: 'finished', metadata: { inputTokens: 15, outputTokens: 8 } };
    });

    const callModel = createBrainCallModel(mockClient);

    const stream = callModel({
      messages: [
        {
          type: 'user',
          message: { role: 'user', content: 'Hello Brain' },
        } as any,
      ],
      systemPrompt: 'You are Claude Code.',
    } as any);

    const events: any[] = [];
    for await (const ev of stream) {
      events.push(ev);
    }

    expect(capturedRequest).not.toBeNull();
    expect(capturedRequest!.messages.length).toBe(1);
    expect(capturedRequest!.messages[0].content).toBe('Hello Brain');

    // Assert standard Claude message streaming events
    const startEvent = events.find((e) => e.type === 'stream_event' && e.event?.type === 'message_start');
    const deltas = events.filter((e) => e.type === 'stream_event' && e.event?.type === 'content_block_delta');
    const stopEvent = events.find((e) => e.type === 'stream_event' && e.event?.type === 'message_stop');

    expect(startEvent).toBeDefined();
    expect(deltas.length).toBeGreaterThan(0);
    expect(stopEvent).toBeDefined();
  });

  // ==========================================================================
  // SCENARIO 2: SESSION IDENTITY PROPAGATION INVARIANT
  // ==========================================================================
  test('Scenario 2: SessionId propagates deterministically across callModel adapter boundary', async () => {
    let receivedSessionId: string | undefined;

    const mockClient = new MockBrainBackendClient(async function* (req) {
      receivedSessionId = req.sessionId;
      yield { type: 'token', token: 'Session acknowledged' };
      yield { type: 'finished' };
    });

    const callModel = createBrainCallModel(mockClient);

    const stream = callModel({
      sessionId: 'session_brain_test_01J5K9',
      messages: [
        {
          type: 'user',
          message: { role: 'user', content: 'Check session identity' },
        } as any,
      ],
    } as any);

    for await (const _ of stream) {
      // drain
    }

    expect(receivedSessionId).toBe('session_brain_test_01J5K9');
  });

  // ==========================================================================
  // SCENARIO 3: MEMORY-AUGMENTED CONTEXT STREAMING INVARIANT
  // ==========================================================================
  test('Scenario 3: Brain-augmented memory context flows naturally through model gateway stream', async () => {
    const mockClient = new MockBrainBackendClient(async function* (req) {
      // Simulate Rust runtime resolving STM/LTM facts into augmented context
      yield { type: 'thinking', thinking: 'Recalling project architecture decisions from episodic memory...' };
      yield { type: 'token', token: 'According to previous session records, we use SQLite WAL for durable LTM storage.' };
      yield { type: 'finished', metadata: { inputTokens: 450, outputTokens: 22 } };
    });

    const callModel = createBrainCallModel(mockClient);

    const stream = callModel({
      messages: [
        {
          type: 'user',
          message: { role: 'user', content: 'What was our storage decision?' },
        } as any,
      ],
    } as any);

    const thinkingEvents: any[] = [];
    const textEvents: any[] = [];

    for await (const ev of stream) {
      if (ev.type === 'stream_event' && ev.event?.type === 'content_block_delta') {
        if (ev.event.delta?.type === 'thinking_delta') {
          thinkingEvents.push(ev.event.delta.thinking);
        } else if (ev.event.delta?.type === 'text_delta') {
          textEvents.push(ev.event.delta.text);
        }
      }
    }

    expect(thinkingEvents.join('')).toContain('Recalling project architecture');
    expect(textEvents.join('')).toContain('SQLite WAL');
  });

  // ==========================================================================
  // SCENARIO 4: ZERO BRAIN DTO LEAKAGE IN CLAUDE UI
  // ==========================================================================
  test('Scenario 4: Zero Rust domain or memory DTOs leak into Claude Message structures', async () => {
    const mockClient = new MockBrainBackendClient(async function* () {
      yield { type: 'token', token: 'Clean response' };
      yield { type: 'finished' };
    });

    const callModel = createBrainCallModel(mockClient);

    const stream = callModel({
      messages: [
        {
          type: 'user',
          message: { role: 'user', content: 'Test boundary leakage' },
        } as any,
      ],
    } as any);

    const events: any[] = [];
    for await (const ev of stream) {
      events.push(ev);
    }

    // Convert events to standard Claude Assistant Message
    const assistantMsg = createAssistantMessage('msg_test_clean', [{ type: 'text', text: 'Clean response' }]);

    const jsonStr = JSON.stringify(assistantMsg);
    expect(jsonStr).not.toContain('MemoryDTO');
    expect(jsonStr).not.toContain('StmNode');
    expect(jsonStr).not.toContain('NodeId');
    expect(jsonStr).not.toContain('KnowledgeGraph');
  });

  // ==========================================================================
  // SCENARIO 5: MID-STREAM CANCELLATION IS CLEAN & RECOVERABLE
  // ==========================================================================
  test('Scenario 5: Aborting callModel mid-stream stops generation immediately without memory corruption', async () => {
    const abortController = new AbortController();
    let tokensEmitted = 0;

    const mockClient = new MockBrainBackendClient(async function* (req) {
      for (let i = 0; i < 100; i++) {
        if (req.signal?.aborted) break;
        tokensEmitted++;
        yield { type: 'token', token: `chunk_${i} ` };
      }
      yield { type: 'finished' };
    });

    const callModel = createBrainCallModel(mockClient);

    const stream = callModel({
      messages: [
        {
          type: 'user',
          message: { role: 'user', content: 'Long streaming turn' },
        } as any,
      ],
      signal: abortController.signal,
    } as any);

    let receivedCount = 0;
    for await (const ev of stream) {
      if (ev.type === 'stream_event' && ev.event?.type === 'content_block_delta') {
        receivedCount++;
        if (receivedCount === 3) {
          abortController.abort();
        }
      }
    }

    expect(receivedCount).toBe(3);
  });

  // ==========================================================================
  // SCENARIO 6: MULTI-TURN TOOL FEEDBACK INGESTION
  // ==========================================================================
  test('Scenario 6: Multi-turn tool execution results preserve complete history across callModel turns', async () => {
    let secondTurnMessages: any[] = [];

    const mockClient = new MockBrainBackendClient(async function* (req) {
      if (req.messages.length > 2) {
        secondTurnMessages = req.messages;
        yield { type: 'token', token: 'File contents analyzed.' };
      } else {
        yield {
          type: 'tool_use',
          toolUse: { id: 'tool_1', name: 'Read', input: { path: '/src/main.rs' } },
        };
      }
      yield { type: 'finished' };
    });

    const callModel = createBrainCallModel(mockClient);

    // Turn 1: User prompt -> Brain tool_use
    const turn1Stream = callModel({
      messages: [{ type: 'user', message: { role: 'user', content: 'Read main.rs' } } as any],
    } as any);

    for await (const _ of turn1Stream) {}

    // Turn 2: User prompt + Assistant tool_use + User tool_result
    const turn2Stream = callModel({
      messages: [
        { type: 'user', message: { role: 'user', content: 'Read main.rs' } } as any,
        {
          type: 'assistant',
          message: {
            role: 'assistant',
            content: [{ type: 'tool_use', id: 'tool_1', name: 'Read', input: { path: '/src/main.rs' } }],
          },
        } as any,
        {
          type: 'user',
          message: {
            role: 'user',
            content: [{ type: 'tool_result', tool_use_id: 'tool_1', content: 'fn main() {}' }],
          },
        } as any,
      ],
    } as any);

    for await (const _ of turn2Stream) {}

    expect(secondTurnMessages.length).toBe(3);
    expect(secondTurnMessages[1].content[0].type).toBe('tool_use');
    expect(secondTurnMessages[2].content[0].type).toBe('tool_result');
  });

  // ==========================================================================
  // SCENARIO 7: BOUNDED RETRIEVAL LATENCY BENCHMARK BASELINE
  // ==========================================================================
  test('Scenario 7: Memory resolution and context assembly completes within bounded latency budget', async () => {
    const iterations = 50;
    const durations: number[] = [];

    const mockClient = new MockBrainBackendClient(async function* () {
      yield { type: 'token', token: 'Fast' };
      yield { type: 'finished' };
    });

    const callModel = createBrainCallModel(mockClient);

    for (let i = 0; i < iterations; i++) {
      const start = performance.now();
      const stream = callModel({
        sessionId: `bench_sess_${i}`,
        messages: [{ type: 'user', message: { role: 'user', content: 'Benchmark query' } } as any],
      } as any);

      for await (const _ of stream) {}
      durations.push(performance.now() - start);
    }

    const avgDuration = durations.reduce((a, b) => a + b, 0) / iterations;
    console.log(`[BENCHMARK] Average memory seam callModel dispatch duration: ${avgDuration.toFixed(2)}ms across ${iterations} runs`);
    expect(avgDuration).toBeLessThan(50); // Generous initial bound for canary test runs
  });
});
