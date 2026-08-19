import { describe, test, expect, beforeAll, afterAll } from 'bun:test';
import * as net from 'net';
import * as fs from 'fs';
import * as path from 'path';
import * as readline from 'readline';
import { query } from '../../vendor/claude/query.js';
import { productionDeps, type QueryDeps } from '../../vendor/claude/query/deps.js';
import { createUserMessage } from '../../vendor/claude/utils/messages.js';
import type { ToolUseContext } from '../../vendor/claude/Tool.js';
import { createBrainCallModel } from '../adapter/brainCallModel.js';
import { UdsBrainBackendClient } from '../client/UdsBrainBackendClient.js';

function createMockToolUseContext(): ToolUseContext {
  const appState: any = {
    toolPermissionContext: { additionalWorkingDirectories: new Map(), alwaysAllowRules: {} },
    mcp: { clients: [], tools: [] },
    sessionHooks: new Map(),
    fastMode: false,
    effortValue: 'high',
  };
  return {
    abortController: new AbortController(),
    agentId: 'perf_agent' as any,
    readFileState: { get: () => null, set: () => {}, has: () => false, delete: () => {} } as any,
    options: {
      tools: [],
      mcpClients: [],
      mainLoopModel: 'claude-3-7-sonnet-20250219',
      thinkingConfig: { mode: 'adaptive' },
      agentDefinitions: { activeAgents: [], allowedAgentTypes: [] },
    },
    getAppState: () => appState,
    setAppState: () => {},
    setInProgressToolUseIDs: () => {},
    setResponseLength: () => {},
    updateFileHistoryState: () => {},
    addNotification: () => {},
  } as any;
}

describe('Phase 6.4: Performance, Latency & RSS Memory Profiling', () => {
  const socketPath = path.join('/tmp', `brain_perf_${Date.now()}_${Math.random().toString(36).slice(2, 6)}.sock`);
  let server: net.Server | null = null;
  let activeHandler: ((socket: net.Socket, req: any) => void) | null = null;

  beforeAll(async () => {
    if (fs.existsSync(socketPath)) {
      try {
        fs.unlinkSync(socketPath);
      } catch {}
    }

    server = net.createServer((socket) => {
      socket.on('error', () => {});
      const rl = readline.createInterface({ input: socket, crlfDelay: Infinity });
      rl.on('line', (line) => {
        if (!line.trim()) return;
        try {
          const req = JSON.parse(line);
          if (activeHandler) {
            activeHandler(socket, req);
          }
        } catch {}
      });
    });

    await new Promise<void>((resolve) => {
      server!.listen(socketPath, () => resolve());
    });
  });

  afterAll(() => {
    if (server) {
      server.close();
    }
    if (fs.existsSync(socketPath)) {
      try {
        fs.unlinkSync(socketPath);
      } catch {}
    }
  });

  test('Benchmark 1: Measure Time-to-First-Token (TTFT) over live UDS connection', async () => {
    activeHandler = (socket) => {
      socket.write(JSON.stringify({ type: 'token', token: 'FirstToken' }) + '\n', () => {});
      for (let i = 0; i < 10; i++) {
        socket.write(JSON.stringify({ type: 'token', token: ` tok_${i}` }) + '\n', () => {});
      }
      socket.write(JSON.stringify({ type: 'finished' }) + '\n', () => {});
    };

    const client = new UdsBrainBackendClient(socketPath);
    const deps: QueryDeps = {
      ...productionDeps(),
      callModel: createBrainCallModel(client),
    };

    const startTime = performance.now();
    let ttftMs = 0;
    let firstTokenReceived = false;

    const stream = query({
      messages: [createUserMessage({ content: 'Benchmark TTFT' })],
      systemPrompt: 'System' as any,
      userContext: {},
      systemContext: {},
      canUseTool: async () => ({ behavior: 'allow' }),
      toolUseContext: createMockToolUseContext(),
      querySource: 'repl',
      deps,
    });

    for await (const event of stream) {
      if (!firstTokenReceived && (event as any).type === 'stream_event') {
        ttftMs = performance.now() - startTime;
        firstTokenReceived = true;
      }
    }

    console.log(`\n[PERF] Measured Time-to-First-Token (TTFT): ${ttftMs.toFixed(2)} ms`);
    expect(firstTokenReceived).toBe(true);
    expect(ttftMs).toBeLessThan(100); // Target < 100ms
  });

  test('Benchmark 2: Measure Stream Throughput and Peak Process RSS Memory', async () => {
    const tokenCount = 1000;
    activeHandler = (socket) => {
      for (let i = 0; i < tokenCount; i++) {
        socket.write(JSON.stringify({ type: 'token', token: 'a' }) + '\n', () => {});
      }
      socket.write(JSON.stringify({ type: 'finished' }) + '\n', () => {});
    };

    const client = new UdsBrainBackendClient(socketPath);
    const deps: QueryDeps = {
      ...productionDeps(),
      callModel: createBrainCallModel(client),
    };

    const startTime = performance.now();
    const stream = query({
      messages: [createUserMessage({ content: 'Benchmark throughput' })],
      systemPrompt: 'System' as any,
      userContext: {},
      systemContext: {},
      canUseTool: async () => ({ behavior: 'allow' }),
      toolUseContext: createMockToolUseContext(),
      querySource: 'repl',
      deps,
    });

    for await (const _ of stream) {}
    const totalDurationMs = performance.now() - startTime;
    const tokensPerSec = (tokenCount / (totalDurationMs / 1000));
    const memUsage = process.memoryUsage();
    const rssMB = memUsage.rss / (1024 * 1024);

    console.log(`[PERF] Streamed ${tokenCount} tokens in ${totalDurationMs.toFixed(2)} ms (${tokensPerSec.toFixed(0)} tokens/sec)`);
    console.log(`[PERF] Peak RSS Memory Usage: ${rssMB.toFixed(2)} MB`);

    expect(rssMB).toBeLessThan(200); // Healthy process memory
  });
});
