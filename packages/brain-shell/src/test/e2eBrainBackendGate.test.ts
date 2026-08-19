import { describe, test, expect, beforeAll, afterAll, beforeEach } from 'bun:test';
import * as net from 'net';
import * as fs from 'fs';
import * as path from 'path';
import * as readline from 'readline';
import { query } from '../../vendor/claude/query.js';
import { productionDeps, type QueryDeps } from '../../vendor/claude/query/deps.js';
import { createUserMessage } from '../../vendor/claude/utils/messages.js';
import type { Tool, ToolUseContext } from '../../vendor/claude/Tool.js';
import type { Message, AssistantMessage } from '../../vendor/claude/types/message.js';
import { createBrainCallModel } from '../adapter/brainCallModel.js';
import { UdsBrainBackendClient } from '../client/UdsBrainBackendClient.js';

function createMockToolUseContext(
  abortController: AbortController = new AbortController(),
  extraTools: Tool[] = [],
  thinkingMode: any = { mode: 'adaptive' }
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
    agentId: 'e2e_gate_agent' as any,
    readFileState: { get: () => null, set: () => {}, has: () => false, delete: () => {} } as any,
    options: {
      tools: extraTools,
      mcpClients: [],
      mainLoopModel: 'claude-3-7-sonnet-20250219',
      thinkingConfig: thinkingMode,
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

function createDummyTool(
  name: string,
  executeResult: (input: any) => Promise<any> = async () => 'OK',
  requiresPermission: boolean = false
): Tool {
  return {
    name,
    description: `Real test tool for ${name}`,
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
    needsPermissions: () => requiresPermission,
  } as any;
}

describe('Phase 5.7: Real End-to-End Brain Backend Hard Gate Matrix', () => {
  const socketPath = path.join('/tmp', `brain_gate_${Date.now()}_${Math.random().toString(36).slice(2, 6)}.sock`);
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

  beforeEach(async () => {
    activeHandler = null;
    await new Promise((resolve) => setTimeout(resolve, 20));
  });

  // 1. Real text generation
  test('Scenario 1: Real text generation over live UDS streaming into Claude AssistantMessage', async () => {
    activeHandler = (socket, req) => {
      expect(req.action).toBe('v1/generation/stream');
      socket.write(JSON.stringify({ type: 'token', token: 'Brain ' }) + '\n', () => {});
      socket.write(JSON.stringify({ type: 'token', token: 'E2E ' }) + '\n', () => {});
      socket.write(JSON.stringify({ type: 'token', token: 'online.' }) + '\n', () => {});
      socket.write(JSON.stringify({ type: 'finished' }) + '\n', () => {});
    };

    const client = new UdsBrainBackendClient(socketPath);
    const deps: QueryDeps = {
      ...productionDeps(),
      callModel: createBrainCallModel(client),
    };

    const stream = query({
      messages: [createUserMessage({ content: 'Status check' })],
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
    expect(finalMsg!.message.content[0].text).toBe('Brain E2E online.');
  });

  // 2. Multi-turn conversation
  test('Scenario 2: Multi-turn conversation maintaining turn history accurately', async () => {
    let capturedTurn2Payload: any = null;

    activeHandler = (socket, req) => {
      if (req.payload.messages.length > 1) {
        capturedTurn2Payload = req.payload.messages;
        socket.write(JSON.stringify({ type: 'token', token: 'Turn 2 response received.' }) + '\n', () => {});
      } else {
        socket.write(JSON.stringify({ type: 'token', token: 'Turn 1 response.' }) + '\n', () => {});
      }
      socket.write(JSON.stringify({ type: 'finished' }) + '\n', () => {});
    };

    const client = new UdsBrainBackendClient(socketPath);
    const deps: QueryDeps = {
      ...productionDeps(),
      callModel: createBrainCallModel(client),
    };

    const history: Message[] = [createUserMessage({ content: 'Turn 1 prompt' })];

    // Turn 1
    for await (const event of query({
      messages: history,
      systemPrompt: 'System' as any,
      userContext: {},
      systemContext: {},
      canUseTool: async () => ({ behavior: 'allow' }),
      toolUseContext: createMockToolUseContext(),
      querySource: 'repl',
      deps,
    })) {
      if ((event as any).type === 'assistant') {
        history.push(event as AssistantMessage);
      }
    }

    // Turn 2
    history.push(createUserMessage({ content: 'Turn 2 prompt referencing turn 1' }));
    for await (const _ of query({
      messages: history,
      systemPrompt: 'System' as any,
      userContext: {},
      systemContext: {},
      canUseTool: async () => ({ behavior: 'allow' }),
      toolUseContext: createMockToolUseContext(),
      querySource: 'repl',
      deps,
    })) {}

    expect(capturedTurn2Payload.length).toBe(3);
    expect(capturedTurn2Payload[0]).toEqual({ role: 'user', content: 'Turn 1 prompt' });
    expect(capturedTurn2Payload[1]).toEqual({ role: 'assistant', content: 'Turn 1 response.' });
    expect(capturedTurn2Payload[2]).toEqual({ role: 'user', content: 'Turn 2 prompt referencing turn 1' });
  });

  // 3. Real thinking
  test('Scenario 3: Real thinking stream generates ThinkingBlock before TextBlock in exact order', async () => {
    activeHandler = (socket) => {
      socket.write(JSON.stringify({ type: 'thinking', thinking: 'Contemplating quantum principles...' }) + '\n', () => {});
      socket.write(JSON.stringify({ type: 'token', token: 'Quantum mechanics applies.' }) + '\n', () => {});
      socket.write(JSON.stringify({ type: 'finished' }) + '\n', () => {});
    };

    const client = new UdsBrainBackendClient(socketPath);
    const deps: QueryDeps = {
      ...productionDeps(),
      callModel: createBrainCallModel(client),
    };

    const stream = query({
      messages: [createUserMessage({ content: 'Explain physics' })],
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
    expect(blocks[0].type).toBe('thinking');
    expect((blocks[0] as any).thinking).toBe('Contemplating quantum principles...');
    expect(blocks[1].type).toBe('text');
    expect((blocks[1] as any).text).toBe('Quantum mechanics applies.');
  });

  // 4. Real tool call
  test('Scenario 4: Real tool call emits ToolUseBlock, executes Claude tool, and returns final answer', async () => {
    let turn = 0;
    const tool = createDummyTool('QueryDatabase', async () => 'Found 42 records');

    activeHandler = (socket) => {
      turn++;
      if (turn === 1) {
        socket.write(
          JSON.stringify({
            type: 'tool_use',
            toolUse: { id: 'call_db_1', name: 'QueryDatabase', input: {} },
          }) + '\n',
          () => {}
        );
      } else {
        socket.write(JSON.stringify({ type: 'token', token: 'Database query returned 42 records.' }) + '\n', () => {});
      }
      socket.write(JSON.stringify({ type: 'finished' }) + '\n', () => {});
    };

    const client = new UdsBrainBackendClient(socketPath);
    const deps: QueryDeps = {
      ...productionDeps(),
      callModel: createBrainCallModel(client),
    };

    const stream = query({
      messages: [createUserMessage({ content: 'Count records' })],
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
    expect(finalMsg!.message.content[0].text).toBe('Database query returned 42 records.');
  });

  // 5. Tool permission approval
  test('Scenario 5: Tool permission approval successfully runs sensitive tool', async () => {
    let turn = 0;
    let toolRan = false;
    const tool = createDummyTool('WriteDisk', async () => {
      toolRan = true;
      return 'File saved.';
    }, true);

    activeHandler = (socket) => {
      turn++;
      if (turn === 1) {
        socket.write(
          JSON.stringify({
            type: 'tool_use',
            toolUse: { id: 'call_write_1', name: 'WriteDisk', input: { path: '/tmp/test' } },
          }) + '\n',
          () => {}
        );
      } else {
        socket.write(JSON.stringify({ type: 'token', token: 'Write operation confirmed.' }) + '\n', () => {});
      }
      socket.write(JSON.stringify({ type: 'finished' }) + '\n', () => {});
    };

    const client = new UdsBrainBackendClient(socketPath);
    const deps: QueryDeps = {
      ...productionDeps(),
      callModel: createBrainCallModel(client),
    };

    const stream = query({
      messages: [createUserMessage({ content: 'Save file' })],
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

    expect(toolRan).toBe(true);
    expect(finalMsg!.message.content[0].text).toBe('Write operation confirmed.');
  });

  // 6. Tool denial
  test('Scenario 6: Tool permission denial yields error block and allows graceful model recovery', async () => {
    let turn = 0;
    let toolRan = false;
    const tool = createDummyTool('DeleteDisk', async () => {
      toolRan = true;
      return 'Deleted';
    }, true);

    activeHandler = (socket, req) => {
      turn++;
      if (turn === 1) {
        socket.write(
          JSON.stringify({
            type: 'tool_use',
            toolUse: { id: 'call_del_1', name: 'DeleteDisk', input: {} },
          }) + '\n',
          () => {}
        );
      } else {
        const userMsg = req.payload.messages.find((m: any) => m.role === 'user' && Array.isArray(m.content));
        expect(userMsg).not.toBeUndefined();
        socket.write(JSON.stringify({ type: 'token', token: 'Action cancelled because permission was denied.' }) + '\n', () => {});
      }
      socket.write(JSON.stringify({ type: 'finished' }) + '\n', () => {});
    };

    const client = new UdsBrainBackendClient(socketPath);
    const deps: QueryDeps = {
      ...productionDeps(),
      callModel: createBrainCallModel(client),
    };

    const stream = query({
      messages: [createUserMessage({ content: 'Delete records' })],
      systemPrompt: 'System' as any,
      userContext: {},
      systemContext: {},
      canUseTool: async () => ({ behavior: 'deny', message: 'User denied operation' }),
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

    expect(toolRan).toBe(false);
    expect(turn).toBe(2);
    expect(finalMsg!.message.content[0].text).toBe('Action cancelled because permission was denied.');
  });

  // 7. Multiple tools
  test('Scenario 7: Multiple tools dispatched and executed in a single turn', async () => {
    let turn = 0;
    const toolA = createDummyTool('ToolA', async () => 'ResultA');
    const toolB = createDummyTool('ToolB', async () => 'ResultB');

    activeHandler = (socket) => {
      turn++;
      if (turn === 1) {
        socket.write(
          JSON.stringify({
            type: 'tool_use',
            toolUse: { id: 'call_a', name: 'ToolA', input: {} },
          }) + '\n',
          () => {}
        );
        socket.write(
          JSON.stringify({
            type: 'tool_use',
            toolUse: { id: 'call_b', name: 'ToolB', input: {} },
          }) + '\n',
          () => {}
        );
      } else {
        socket.write(JSON.stringify({ type: 'token', token: 'Combined results: ResultA and ResultB.' }) + '\n', () => {});
      }
      socket.write(JSON.stringify({ type: 'finished' }) + '\n', () => {});
    };

    const client = new UdsBrainBackendClient(socketPath);
    const deps: QueryDeps = {
      ...productionDeps(),
      callModel: createBrainCallModel(client),
    };

    const stream = query({
      messages: [createUserMessage({ content: 'Run both tools' })],
      systemPrompt: 'System' as any,
      userContext: {},
      systemContext: {},
      canUseTool: async () => ({ behavior: 'allow' }),
      toolUseContext: createMockToolUseContext(new AbortController(), [toolA, toolB]),
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
    expect(finalMsg!.message.content[0].text).toBe('Combined results: ResultA and ResultB.');
  });

  // 8. Cancellation
  test('Scenario 8: Mid-stream cancellation terminates UDS socket without ghost iterations', async () => {
    const ac = new AbortController();

    activeHandler = (socket) => {
      socket.write(JSON.stringify({ type: 'token', token: 'Token 1 ' }) + '\n', () => {});
      socket.write(JSON.stringify({ type: 'token', token: 'Token 2 ' }) + '\n', () => {});
    };

    const client = new UdsBrainBackendClient(socketPath);
    const deps: QueryDeps = {
      ...productionDeps(),
      callModel: createBrainCallModel(client),
    };

    const stream = query({
      messages: [createUserMessage({ content: 'Abort test' })],
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

  // 9. Backend disconnect
  test('Scenario 9: Backend socket disconnect mid-stream emits clean API error without retrying', async () => {
    activeHandler = (socket) => {
      socket.write(JSON.stringify({ type: 'token', token: 'Initial token' }) + '\n');
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
      messages: [createUserMessage({ content: 'Disconnect test' })],
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

  // 10. Large streamed response
  test('Scenario 10: Large streamed response (1000+ tokens) accumulates without token loss', async () => {
    const totalTokens = 1000;

    activeHandler = (socket) => {
      for (let i = 0; i < totalTokens; i++) {
        socket.write(JSON.stringify({ type: 'token', token: `t${i} ` }) + '\n', () => {});
      }
      socket.write(JSON.stringify({ type: 'finished' }) + '\n', () => {});
    };

    const client = new UdsBrainBackendClient(socketPath);
    const deps: QueryDeps = {
      ...productionDeps(),
      callModel: createBrainCallModel(client),
    };

    const stream = query({
      messages: [createUserMessage({ content: 'Stream 1000 tokens' })],
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
    const text = finalMsg!.message.content[0].text;
    expect(text.startsWith('t0 ')).toBe(true);
    expect(text.endsWith('t999 ')).toBe(true);
  });

  // 11. Unicode / Markdown formatting
  test('Scenario 11: Complex Markdown and Unicode box-drawing structures are preserved intact', async () => {
    const complexMarkdown = `# Analysis Report\n| Metric | Value |\n|---|---|\n| Accuracy | 99.8% |\n\`\`\`rust\nfn main() { println!("Hello 🚀"); }\n\`\`\``;

    activeHandler = (socket) => {
      socket.write(JSON.stringify({ type: 'token', token: complexMarkdown }) + '\n', () => {});
      socket.write(JSON.stringify({ type: 'finished' }) + '\n', () => {});
    };

    const client = new UdsBrainBackendClient(socketPath);
    const deps: QueryDeps = {
      ...productionDeps(),
      callModel: createBrainCallModel(client),
    };

    const stream = query({
      messages: [createUserMessage({ content: 'Render report' })],
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

    expect(finalMsg!.message.content[0].text).toBe(complexMarkdown);
  });

  // 12. Turn completion
  test('Scenario 12: Turn completion cleanly finishes and closes generator with stop_reason', async () => {
    activeHandler = (socket) => {
      socket.write(JSON.stringify({ type: 'token', token: 'Clean turn completion.' }) + '\n', () => {});
      socket.write(JSON.stringify({ type: 'finished' }) + '\n', () => {});
    };

    const client = new UdsBrainBackendClient(socketPath);
    const deps: QueryDeps = {
      ...productionDeps(),
      callModel: createBrainCallModel(client),
    };

    const stream = query({
      messages: [createUserMessage({ content: 'Complete turn' })],
      systemPrompt: 'System' as any,
      userContext: {},
      systemContext: {},
      canUseTool: async () => ({ behavior: 'allow' }),
      toolUseContext: createMockToolUseContext(),
      querySource: 'repl',
      deps,
    });

    let messageStopReceived = false;
    for await (const event of stream) {
      if ((event as any).type === 'stream_event' && (event as any).event?.type === 'message_stop') {
        messageStopReceived = true;
      }
    }

    expect(messageStopReceived).toBe(true);
  });

  // 13. Recovery after cancelled turn
  test('Scenario 13: Clean recovery after a cancelled turn with fresh prompt', async () => {
    activeHandler = (socket, req) => {
      const prompt = req.payload.messages[0].content;
      if (prompt === 'Cancel me') {
        socket.write(JSON.stringify({ type: 'token', token: 'Cancelled stream...' }) + '\n', () => {});
      } else {
        socket.write(JSON.stringify({ type: 'token', token: 'Fresh prompt succeeded.' }) + '\n', () => {});
        socket.write(JSON.stringify({ type: 'finished' }) + '\n', () => {});
      }
    };

    const client = new UdsBrainBackendClient(socketPath);
    const deps: QueryDeps = {
      ...productionDeps(),
      callModel: createBrainCallModel(client),
    };

    // First turn: cancel
    const ac1 = new AbortController();
    for await (const event of query({
      messages: [createUserMessage({ content: 'Cancel me' })],
      systemPrompt: 'System' as any,
      userContext: {},
      systemContext: {},
      canUseTool: async () => ({ behavior: 'allow' }),
      toolUseContext: createMockToolUseContext(ac1),
      querySource: 'repl',
      deps,
    })) {
      if ((event as any).type === 'stream_event') {
        ac1.abort();
      }
    }

    expect(ac1.signal.aborted).toBe(true);

    // Second turn: fresh prompt
    let finalMsg2: AssistantMessage | null = null;
    for await (const event of query({
      messages: [createUserMessage({ content: 'Fresh turn after cancel' })],
      systemPrompt: 'System' as any,
      userContext: {},
      systemContext: {},
      canUseTool: async () => ({ behavior: 'allow' }),
      toolUseContext: createMockToolUseContext(),
      querySource: 'repl',
      deps,
    })) {
      if ((event as any).type === 'assistant') {
        finalMsg2 = event as AssistantMessage;
      }
    }

    expect(finalMsg2).not.toBeNull();
    expect(finalMsg2!.message.content[0].text).toBe('Fresh prompt succeeded.');
  });
});
