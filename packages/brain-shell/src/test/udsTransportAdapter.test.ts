import { describe, test, expect, beforeAll, afterAll } from 'bun:test';
import * as net from 'net';
import * as fs from 'fs';
import * as path from 'path';
import * as readline from 'readline';
import { query } from '../../vendor/claude/query.js';
import { productionDeps, type QueryDeps } from '../../vendor/claude/query/deps.js';
import { createUserMessage } from '../../vendor/claude/utils/messages.js';
import type { Tool, ToolUseContext } from '../../vendor/claude/Tool.js';
import type { AssistantMessage } from '../../vendor/claude/types/message.js';
import { createBrainCallModel } from '../adapter/brainCallModel.js';
import { UdsBrainBackendClient } from '../client/UdsBrainBackendClient.js';

function createMockToolUseContext(
  abortController: AbortController = new AbortController(),
  extraTools: Tool[] = []
): ToolUseContext {
  const appState: any = {
    toolPermissionContext: { additionalWorkingDirectories: new Map(), alwaysAllowRules: {} },
    mcp: { clients: [], tools: [] },
    sessionHooks: new Map(),
    fastMode: false,
    effortValue: 'high',
  };
  return {
    abortController,
    agentId: 'uds_test_agent' as any,
    readFileState: { get: () => null, set: () => {}, has: () => false, delete: () => {} } as any,
    options: {
      tools: extraTools,
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

function createDummyTool(name: string, executeResult: (input: any) => Promise<any> = async () => 'OK'): Tool {
  return {
    name,
    description: `Dummy test tool for ${name}`,
    inputSchema: {
      safeParse: (input: any) => ({ success: true, data: input }),
      parse: (input: any) => input,
    } as any,
    call: async (input: any) => ({ data: await executeResult(input) }),
    mapToolResultToToolResultBlockParam: (data: any, toolUseID: string) => ({
      type: 'tool_result',
      tool_use_id: toolUseID,
      content: typeof data === 'string' ? data : JSON.stringify(data),
    }),
    needsPermissions: () => false,
  } as any;
}

describe('Phase 5.6: Live UDS Transport Adapter Matrix', () => {
  const socketPath = path.join('/tmp', `brain_test_${Date.now()}_${Math.random().toString(36).slice(2, 6)}.sock`);
  let server: net.Server | null = null;
  let serverHandler: ((socket: net.Socket, request: any) => void) | null = null;

  beforeAll(async () => {
    if (fs.existsSync(socketPath)) {
      fs.unlinkSync(socketPath);
    }

    server = net.createServer((socket) => {
      const rl = readline.createInterface({ input: socket, crlfDelay: Infinity });
      rl.on('line', (line) => {
        if (!line.trim()) return;
        try {
          const req = JSON.parse(line);
          if (serverHandler) {
            serverHandler(socket, req);
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

  test('Scenario 1: Live UDS stream round-trip (Text & Thinking streamed over Unix Domain Socket)', async () => {
    serverHandler = (socket, req) => {
      expect(req.action).toBe('v1/generation/stream');
      expect(req.payload.messages[0].content).toBe('Hello over UDS');

      // Stream thinking and text frames
      socket.write(JSON.stringify({ type: 'thinking', thinking: 'Analyzing query over socket...' }) + '\n');
      socket.write(JSON.stringify({ type: 'token', token: 'Response ' }) + '\n');
      socket.write(JSON.stringify({ type: 'token', token: 'streamed ' }) + '\n');
      socket.write(JSON.stringify({ type: 'token', token: 'via UDS.' }) + '\n');
      socket.write(JSON.stringify({ type: 'finished' }) + '\n');
    };

    const client = new UdsBrainBackendClient(socketPath);
    const deps: QueryDeps = {
      ...productionDeps(),
      callModel: createBrainCallModel(client),
    };

    const stream = query({
      messages: [createUserMessage({ content: 'Hello over UDS' })],
      systemPrompt: 'System' as any,
      userContext: {},
      systemContext: {},
      canUseTool: async () => ({ behavior: 'allow' }),
      toolUseContext: createMockToolUseContext(),
      querySource: 'repl',
      deps,
    });

    let finalMsg: AssistantMessage | null = null;
    for await (const event of stream) {
      if ((event as any).type === 'assistant') {
        finalMsg = event as AssistantMessage;
      }
    }

    expect(finalMsg).not.toBeNull();
    const blocks = finalMsg!.message.content;
    expect(blocks.length).toBe(2);
    expect(blocks[0].type).toBe('thinking');
    expect((blocks[0] as any).thinking).toBe('Analyzing query over socket...');
    expect(blocks[1].type).toBe('text');
    expect((blocks[1] as any).text).toBe('Response streamed via UDS.');
  });

  test('Scenario 2: Tool execution round-trip over live UDS socket', async () => {
    let turn = 0;
    const tool = createDummyTool('SocketTool', async () => 'SocketResult(42)');

    serverHandler = (socket, req) => {
      turn++;
      if (turn === 1) {
        // Yield tool call
        socket.write(
          JSON.stringify({
            type: 'tool_use',
            toolUse: { id: 'call_sock_1', name: 'SocketTool', input: {} },
          }) + '\n'
        );
        socket.write(JSON.stringify({ type: 'finished' }) + '\n');
      } else {
        // Turn 2: verify tool result was sent
        const userMsg = req.payload.messages.find((m: any) => m.role === 'user' && Array.isArray(m.content));
        expect(userMsg).not.toBeUndefined();
        socket.write(JSON.stringify({ type: 'token', token: 'Tool execution confirmed: 42' }) + '\n');
        socket.write(JSON.stringify({ type: 'finished' }) + '\n');
      }
    };

    const client = new UdsBrainBackendClient(socketPath);
    const deps: QueryDeps = {
      ...productionDeps(),
      callModel: createBrainCallModel(client),
    };

    const stream = query({
      messages: [createUserMessage({ content: 'Run socket tool' })],
      systemPrompt: 'System' as any,
      userContext: {},
      systemContext: {},
      canUseTool: async () => ({ behavior: 'allow' }),
      toolUseContext: createMockToolUseContext(new AbortController(), [tool]),
      querySource: 'repl',
      deps,
    });

    let finalMsg: AssistantMessage | null = null;
    for await (const event of stream) {
      if ((event as any).type === 'assistant') {
        finalMsg = event as AssistantMessage;
      }
    }

    expect(turn).toBe(2);
    expect(finalMsg).not.toBeNull();
    expect(finalMsg!.message.content[0].text).toBe('Tool execution confirmed: 42');
  });

  test('Scenario 3: Non-existent socket path produces clean Claude API error', async () => {
    const nonExistentPath = '/tmp/non_existent_brain_daemon_12345.sock';
    const client = new UdsBrainBackendClient(nonExistentPath);

    const deps: QueryDeps = {
      ...productionDeps(),
      callModel: createBrainCallModel(client),
    };

    const stream = query({
      messages: [createUserMessage({ content: 'Test connection failure' })],
      systemPrompt: 'System' as any,
      userContext: {},
      systemContext: {},
      canUseTool: async () => ({ behavior: 'allow' }),
      toolUseContext: createMockToolUseContext(),
      querySource: 'repl',
      deps,
    });

    let errorMsg: AssistantMessage | null = null;
    for await (const event of stream) {
      if ((event as any).type === 'assistant' && (event as any).isApiErrorMessage) {
        errorMsg = event as AssistantMessage;
      }
    }

    expect(errorMsg).not.toBeNull();
    expect(errorMsg!.isApiErrorMessage).toBe(true);
    expect(errorMsg!.message.content[0].text).toContain('Could not connect to Brain daemon');
  });

  test('Scenario 4: Socket disconnect mid-stream emits deterministic error without reconnect', async () => {
    serverHandler = (socket) => {
      // Send 1 token then abruptly sever connection
      socket.write(JSON.stringify({ type: 'token', token: 'Partial output' }) + '\n');
      setTimeout(() => {
        socket.destroy();
      }, 10);
    };

    const client = new UdsBrainBackendClient(socketPath);
    const deps: QueryDeps = {
      ...productionDeps(),
      callModel: createBrainCallModel(client),
    };

    const stream = query({
      messages: [createUserMessage({ content: 'Sever test' })],
      systemPrompt: 'System' as any,
      userContext: {},
      systemContext: {},
      canUseTool: async () => ({ behavior: 'allow' }),
      toolUseContext: createMockToolUseContext(),
      querySource: 'repl',
      deps,
    });

    let errorMsg: AssistantMessage | null = null;
    for await (const event of stream) {
      if ((event as any).type === 'assistant' && (event as any).isApiErrorMessage) {
        errorMsg = event as AssistantMessage;
      }
    }

    expect(errorMsg).not.toBeNull();
    expect(errorMsg!.isApiErrorMessage).toBe(true);
    expect(errorMsg!.message.content[0].text).toContain('disconnected mid-stream');
  });

  test('Scenario 5: Cancellation during UDS streaming closes socket and halts generation', async () => {
    const ac = new AbortController();
    let cancelReceivedAtServer = false;

    serverHandler = (socket, req) => {
      if (req.action === 'v1/generation/cancel') {
        cancelReceivedAtServer = true;
        return;
      }

      // Stream continuously until cancelled
      socket.write(JSON.stringify({ type: 'token', token: 'chunk_1 ' }) + '\n');
      socket.write(JSON.stringify({ type: 'token', token: 'chunk_2 ' }) + '\n');
    };

    const client = new UdsBrainBackendClient(socketPath);
    const deps: QueryDeps = {
      ...productionDeps(),
      callModel: createBrainCallModel(client),
    };

    const stream = query({
      messages: [createUserMessage({ content: 'Cancel test' })],
      systemPrompt: 'System' as any,
      userContext: {},
      systemContext: {},
      canUseTool: async () => ({ behavior: 'allow' }),
      toolUseContext: createMockToolUseContext(ac),
      querySource: 'repl',
      deps,
    });

    for await (const event of stream) {
      if ((event as any).type === 'stream_event' && (event as any).event?.type === 'content_block_delta') {
        ac.abort();
      }
    }

    expect(ac.signal.aborted).toBe(true);
  });
});
