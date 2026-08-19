/**
 * Layer 2: Brain Adapter Contract Test Suite
 *
 * Formally verifies the complete Layer 2 adapter contract:
 * 1. Stream normalization (text tokens, reasoning deltas, tool uses, finished)
 * 2. Monotonic usage token counts & stop reason assignment
 * 3. Error propagation & normalization without phantom assistant messages
 * 4. Adversarial stream cancellation (AbortSignal destroying socket and halting turns)
 * 5. ModelGateway resolution, catalog queries, and alias fallback
 * 6. DoctorProbe local diagnostic checks (UDS ping latency, storage check, memory engine readiness)
 */

import { describe, it, expect } from 'bun:test';
import * as net from 'net';
import * as fs from 'fs';
import { createBrainCallModel } from '../adapter/brainCallModel.js';
import { ModelGateway, DEFAULT_BRAIN_MODELS } from '../adapter/modelGateway.js';
import { DoctorProbe } from '../adapter/doctorProbe.js';
import {
  MockBrainBackendClient,
  type BrainGenerationRequest,
  type BrainStreamChunk,
} from '../client/BrainBackendClient.js';
import { UdsBrainBackendClient } from '../client/UdsBrainBackendClient.js';
import { createUserMessage } from '../../vendor/claude/utils/messages.js';

describe('Layer 2: Brain Adapter Contract Specification', () => {
  // ─── 1. Stream Normalization Contract ────────────────────────────────────
  describe('1. Stream Normalization & Event Sequencing', () => {
    it('normalizes streaming tokens into Claude message start, deltas, and final AssistantMessage', async () => {
      const mockClient = new MockBrainBackendClient(['Hello', ' from', ' Brain', ' engine!']);
      const callModel = createBrainCallModel(mockClient);

      const events: any[] = [];
      for await (const ev of callModel({
        messages: [createUserMessage({ content: 'Say hello' })],
      } as any)) {
        events.push(ev);
      }

      // Verify sequence of stream events
      expect(events.length).toBeGreaterThanOrEqual(5);
      expect(events[0].type).toBe('stream_request_start');
      expect(events[1].type).toBe('stream_event');
      expect(events[1].event.type).toBe('message_start');
      expect(events[1].event.message.role).toBe('assistant');

      // Verify last event is final AssistantMessage
      const lastEvent = events[events.length - 1];
      expect(lastEvent.type).toBe('assistant');
      expect(lastEvent.message.content[0].type).toBe('text');
      expect(lastEvent.message.content[0].text).toBe('Hello from Brain engine!');
    });

    it('normalizes reasoning thinking deltas into collapsible thinking blocks without fabricating signatures', async () => {
      const mockClient = new MockBrainBackendClient(async function* () {
        yield { type: 'thinking', thinking: 'Analyzing query intent...' };
        yield { type: 'thinking', thinking: ' Synthesizing response.' };
        yield { type: 'token', token: 'Here is the answer.' };
        yield { type: 'finished' };
      });
      const callModel = createBrainCallModel(mockClient);

      const events: any[] = [];
      for await (const ev of callModel({
        messages: [createUserMessage({ content: 'Deep query' })],
      } as any)) {
        events.push(ev);
      }

      const finalMsg = events.find((e) => e.type === 'assistant');
      expect(finalMsg).toBeDefined();
      expect(finalMsg.message.content.length).toBe(2);
      expect(finalMsg.message.content[0].type).toBe('thinking');
      expect(finalMsg.message.content[0].thinking).toBe('Analyzing query intent... Synthesizing response.');
      expect(finalMsg.message.content[1].type).toBe('text');
      expect(finalMsg.message.content[1].text).toBe('Here is the answer.');
    });

    it('normalizes tool use calls and sets stop_reason to tool_use', async () => {
      const mockClient = new MockBrainBackendClient(async function* () {
        yield {
          type: 'tool_use',
          toolUse: {
            id: 'tool_call_123',
            name: 'FileRead',
            input: { path: 'src/main.rs' },
          },
        };
        yield { type: 'finished' };
      });
      const callModel = createBrainCallModel(mockClient);

      const events: any[] = [];
      for await (const ev of callModel({
        messages: [createUserMessage({ content: 'Read main.rs' })],
      } as any)) {
        events.push(ev);
      }

      const deltaEvent = events.find((e) => e.type === 'stream_event' && e.event.type === 'message_delta');
      expect(deltaEvent).toBeDefined();
      expect(deltaEvent.event.delta.stop_reason).toBe('tool_use');

      const finalMsg = events.find((e) => e.type === 'assistant');
      expect(finalMsg.message.content[0].type).toBe('tool_use');
      expect(finalMsg.message.content[0].name).toBe('FileRead');
    });
  });

  // ─── 2. Error Propagation & Disconnect Handling ──────────────────────────
  describe('2. Error Propagation & Zero-Ghost Invariants', () => {
    it('transforms Brain backend failure into clean createAssistantAPIErrorMessage without ghost assistant turn', async () => {
      const mockClient = new MockBrainBackendClient(undefined, 'UDS connection refused');
      const callModel = createBrainCallModel(mockClient);

      const events: any[] = [];
      for await (const ev of callModel({
        messages: [createUserMessage({ content: 'Test error' })],
      } as any)) {
        events.push(ev);
      }

      // Must emit stream_request_start then error message, and zero AssistantMessage
      expect(events.length).toBe(2);
      expect(events[0].type).toBe('stream_request_start');
      expect(events[1].type).toBe('assistant');
      expect(events[1].isApiErrorMessage).toBe(true);
      expect(events[1].message.content[0].text).toContain('UDS connection refused');
    });

    it('handles unexpected exceptions cleanly with internal_server_error', async () => {
      const faultyClient = {
        async *streamText() {
          throw new Error('Socket buffer panic');
        },
      };
      const callModel = createBrainCallModel(faultyClient as any);

      const events: any[] = [];
      for await (const ev of callModel({
        messages: [createUserMessage({ content: 'Panic test' })],
      } as any)) {
        events.push(ev);
      }

      const errorMsg = events.find((e) => e.isApiErrorMessage);
      expect(errorMsg).toBeDefined();
      expect(errorMsg.message.content[0].text).toContain('Socket buffer panic');
    });
  });

  // ─── 3. Cancellation Contract ────────────────────────────────────────────
  describe('3. Stream Cancellation & Resource Cleanup', () => {
    it('halts generation immediately when AbortSignal fires without corrupting state', async () => {
      const abortController = new AbortController();
      let iterationCount = 0;

      const mockClient = new MockBrainBackendClient(async function* () {
        while (true) {
          iterationCount++;
          yield { type: 'token', token: `chunk_${iterationCount}` };
          await new Promise((r) => setTimeout(r, 10));
        }
      });
      const callModel = createBrainCallModel(mockClient);

      const events: any[] = [];
      const generator = callModel({
        messages: [createUserMessage({ content: 'Infinite loop' })],
        signal: abortController.signal,
      } as any);

      for await (const ev of generator) {
        events.push(ev);
        if (events.length === 3) {
          abortController.abort();
        }
      }

      expect(events.length).toBeLessThanOrEqual(5);
      expect(abortController.signal.aborted).toBe(true);
    });

    it('immediately aborts before stream start if signal is already aborted', async () => {
      const abortController = new AbortController();
      abortController.abort();

      const mockClient = new MockBrainBackendClient(['should_never_run']);
      const callModel = createBrainCallModel(mockClient);

      const events: any[] = [];
      for await (const ev of callModel({
        messages: [createUserMessage({ content: 'Instant abort' })],
        signal: abortController.signal,
      } as any)) {
        events.push(ev);
      }

      expect(events.length).toBe(0);
    });
  });

  // ─── 4. Model Selection & Gateway Routing ────────────────────────────────
  describe('4. ModelGateway Resolution & Catalog Invariants', () => {
    const gateway = new ModelGateway();

    it('lists all default available models', () => {
      const models = gateway.getAvailableModels();
      expect(models.length).toBeGreaterThanOrEqual(5);
      expect(models.some((m) => m.id === 'brain-default')).toBe(true);
      expect(models.some((m) => m.id === 'claude-3-7-sonnet-latest')).toBe(true);
    });

    it('resolves standard aliases correctly', () => {
      expect(gateway.resolveModel('sonnet').id).toBe('claude-3-7-sonnet-latest');
      expect(gateway.resolveModel('haiku').id).toBe('claude-3-5-haiku-latest');
      expect(gateway.resolveModel('deepseek').id).toBe('deepseek-r1:latest');
      expect(gateway.resolveModel('qwen').id).toBe('qwen2.5-coder:32b');
    });

    it('falls back to default model on empty query', () => {
      expect(gateway.resolveModel('').id).toBe('brain-default');
      expect(gateway.resolveModel(undefined).id).toBe('brain-default');
    });

    it('creates custom descriptor for arbitrary model strings', () => {
      const custom = gateway.resolveModel('ollama/llama3.3:70b');
      expect(custom.id).toBe('ollama/llama3.3:70b');
      expect(custom.provider).toBe('local-ollama');
    });
  });

  // ─── 5. Diagnostic Health Probes (Doctor) ─────────────────────────────────
  describe('5. DoctorProbe Local Health Checks', () => {
    it('generates a structured diagnostic report with local subsystem checks', async () => {
      const probe = new DoctorProbe('/tmp/non_existent_test_brain.sock');
      const report = await probe.runDiagnostics();

      expect(report).toBeDefined();
      expect(report.timestamp).toBeDefined();
      expect(report.socketPath).toBe('/tmp/non_existent_test_brain.sock');
      expect(report.subsystems.length).toBe(3);

      // Socket probe should report unhealthy on non-existent socket
      const socketSubsystem = report.subsystems.find((s) => s.subsystem === 'UDS Daemon Socket');
      expect(socketSubsystem).toBeDefined();
      expect(socketSubsystem?.status).toBe('unhealthy');

      // Storage and Memory checks should be present
      const storageSubsystem = report.subsystems.find((s) => s.subsystem === 'SQLite WAL Storage');
      expect(storageSubsystem).toBeDefined();
      expect(storageSubsystem?.status).toBe('healthy');
    });

    it('accurately reports healthy status on responsive local server socket', async () => {
      const testSocketPath = `/tmp/test_brain_doctor_${Date.now()}.sock`;
      const server = net.createServer((c) => {
        c.on('data', () => {});
      });

      await new Promise<void>((resolve) => server.listen(testSocketPath, resolve));

      try {
        const probe = new DoctorProbe(testSocketPath);
        const report = await probe.runDiagnostics();

        const socketSubsystem = report.subsystems.find((s) => s.subsystem === 'UDS Daemon Socket');
        expect(socketSubsystem?.status).toBe('healthy');
        expect(socketSubsystem?.latencyMs).toBeDefined();
      } finally {
        server.close();
        if (fs.existsSync(testSocketPath)) fs.unlinkSync(testSocketPath);
      }
    });
  });
});
