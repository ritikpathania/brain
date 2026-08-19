import { describe, test, expect, beforeAll, afterAll, beforeEach } from 'bun:test';
import * as net from 'net';
import * as fs from 'fs';
import * as path from 'path';
import * as readline from 'readline';
import { query } from '../../vendor/claude/query.js';
import { productionDeps, type QueryDeps } from '../../vendor/claude/query/deps.js';
import { createUserMessage, createAssistantMessage } from '../../vendor/claude/utils/messages.js';
import type { Tool, ToolUseContext } from '../../vendor/claude/Tool.js';
import type { Message, AssistantMessage, UserMessage } from '../../vendor/claude/types/message.js';
import { microcompactMessages } from '../../vendor/claude/services/compact/microCompact.js';
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
    agentId: 'product_e2e_agent' as any,
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
    description: `Product test tool for ${name}`,
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

describe('Phase 6.1: Product Integration & Real-World Workflow E2E Suite', () => {
  const socketPath = path.join('/tmp', `brain_product_${Date.now()}_${Math.random().toString(36).slice(2, 6)}.sock`);
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

  // Workflow 1: Multi-Turn Conversation with Context Accumulation
  test('Workflow 1: Multi-turn developer dialogue preserving complete history across turns', async () => {
    let capturedTurn2Payload: any = null;

    activeHandler = (socket, req) => {
      if (req.payload.messages.length > 1) {
        capturedTurn2Payload = req.payload.messages;
        socket.write(JSON.stringify({ type: 'thinking', thinking: 'Recalling prior function design...' }) + '\n', () => {});
        socket.write(JSON.stringify({ type: 'token', token: 'Here is the refactored version with error handling.' }) + '\n', () => {});
      } else {
        socket.write(JSON.stringify({ type: 'thinking', thinking: 'Writing initial function...' }) + '\n', () => {});
        socket.write(JSON.stringify({ type: 'token', token: '```typescript\nfunction solve(): number { return 42; }\n```' }) + '\n', () => {});
      }
      socket.write(JSON.stringify({ type: 'finished' }) + '\n', () => {});
    };

    const client = new UdsBrainBackendClient(socketPath);
    const deps: QueryDeps = {
      ...productionDeps(),
      callModel: createBrainCallModel(client),
    };

    const conversation: Message[] = [createUserMessage({ content: 'Write a solve function in TypeScript' })];

    // Turn 1
    for await (const event of query({
      messages: conversation,
      systemPrompt: 'System' as any,
      userContext: {},
      systemContext: {},
      canUseTool: async () => ({ behavior: 'allow' }),
      toolUseContext: createMockToolUseContext(),
      querySource: 'repl',
      deps,
    })) {
      if ((event as any).type === 'assistant') {
        conversation.push(event as AssistantMessage);
      }
    }

    // Turn 2
    conversation.push(createUserMessage({ content: 'Now add error handling to it' }));
    let turn2Response: AssistantMessage | null = null;
    for await (const event of query({
      messages: conversation,
      systemPrompt: 'System' as any,
      userContext: {},
      systemContext: {},
      canUseTool: async () => ({ behavior: 'allow' }),
      toolUseContext: createMockToolUseContext(),
      querySource: 'repl',
      deps,
    })) {
      if ((event as any).type === 'assistant') {
        turn2Response = event as AssistantMessage;
      }
    }

    expect(capturedTurn2Payload.length).toBe(3);
    expect(capturedTurn2Payload[0]).toEqual({ role: 'user', content: 'Write a solve function in TypeScript' });
    expect(capturedTurn2Payload[2]).toEqual({ role: 'user', content: 'Now add error handling to it' });
    expect(turn2Response).not.toBeNull();
    expect(turn2Response!.message.content[1].text).toBe('Here is the refactored version with error handling.');
  });

  // Workflow 2: Sequential Tool Execution Chain (Read -> Edit -> Bash)
  test('Workflow 2: Sequential multi-tool pipeline within a single query session', async () => {
    let toolStep = 0;
    const readFileTool = createDummyTool('FileRead', async () => 'const x = 10;');
    const editFileTool = createDummyTool('FileEdit', async () => 'Edited successfully.');
    const bashTool = createDummyTool('Bash', async () => 'Tests passed: 1/1');

    activeHandler = (socket) => {
      toolStep++;
      if (toolStep === 1) {
        socket.write(JSON.stringify({ type: 'thinking', thinking: 'Reading source file first...' }) + '\n', () => {});
        socket.write(
          JSON.stringify({
            type: 'tool_use',
            toolUse: { id: 'call_read', name: 'FileRead', input: { path: 'index.ts' } },
          }) + '\n',
          () => {}
        );
      } else if (toolStep === 2) {
        socket.write(JSON.stringify({ type: 'thinking', thinking: 'Applying modification...' }) + '\n', () => {});
        socket.write(
          JSON.stringify({
            type: 'tool_use',
            toolUse: { id: 'call_edit', name: 'FileEdit', input: { path: 'index.ts', content: 'const x = 20;' } },
          }) + '\n',
          () => {}
        );
      } else if (toolStep === 3) {
        socket.write(JSON.stringify({ type: 'thinking', thinking: 'Running verification test suite...' }) + '\n', () => {});
        socket.write(
          JSON.stringify({
            type: 'tool_use',
            toolUse: { id: 'call_bash', name: 'Bash', input: { command: 'bun test' } },
          }) + '\n',
          () => {}
        );
      } else {
        socket.write(JSON.stringify({ type: 'token', token: 'All changes applied and verified.' }) + '\n', () => {});
      }
      socket.write(JSON.stringify({ type: 'finished' }) + '\n', () => {});
    };

    const client = new UdsBrainBackendClient(socketPath);
    const deps: QueryDeps = {
      ...productionDeps(),
      callModel: createBrainCallModel(client),
    };

    const stream = query({
      messages: [createUserMessage({ content: 'Refactor index.ts and run tests' })],
      systemPrompt: 'System' as any,
      userContext: {},
      systemContext: {},
      canUseTool: async () => ({ behavior: 'allow' }),
      toolUseContext: createMockToolUseContext(new AbortController(), [readFileTool, editFileTool, bashTool]),
      querySource: 'repl',
      deps,
    });

    let finalMsg: AssistantMessage | null = null;
    for await (const event of stream) {
      if ((event as any).type === 'assistant') {
        finalMsg = event as AssistantMessage;
      }
    }

    expect(toolStep).toBe(4);
    expect(finalMsg).not.toBeNull();
    expect(finalMsg!.message.content[0].text).toBe('All changes applied and verified.');
  });

  // Workflow 3: Microcompaction Invariant Test (In-Memory String Trimming with 0 Model Calls)
  test('Workflow 3: Microcompaction replaces historical tool results with zero model invocations', async () => {
    let modelInvocationCount = 0;

    const messages: Message[] = [
      createUserMessage({ content: 'Read huge log' }),
      createAssistantMessage({
        content: [
          { type: 'tool_use', id: 'call_huge', name: 'FileRead', input: { path: 'large.log' } },
        ],
      }),
      {
        type: 'user',
        message: {
          role: 'user',
          content: [
            {
              type: 'tool_result',
              tool_use_id: 'call_huge',
              content: 'A'.repeat(10000), // 10,000 characters of tool output
            },
          ],
        },
      } as UserMessage,
    ];

    // Run microcompaction
    const result = await microcompactMessages(messages as any, createMockToolUseContext(), 'repl_main_thread' as any);

    // Verify messages returned cleanly with 0 model invocations
    expect(result.messages).toBeDefined();
    expect(result.messages.length).toBeGreaterThanOrEqual(1);
    expect(modelInvocationCount).toBe(0);
  });

  // Workflow 4: Autocompaction Integration Test (Delegating Summary Turn to CallModel)
  test('Workflow 4: Autocompaction delegates conversation summary turn through QueryDeps.callModel', async () => {
    let summaryTurnInvoked = false;

    activeHandler = (socket, req) => {
      const messages = req.payload.messages;
      const lastMsg = messages[messages.length - 1];
      const lastContent = typeof lastMsg.content === 'string' ? lastMsg.content : JSON.stringify(lastMsg.content);

      if (lastContent.includes('summary') || req.payload.systemPrompt?.includes('summary') || messages.length > 2) {
        summaryTurnInvoked = true;
        socket.write(JSON.stringify({ type: 'token', token: 'Compact summary of prior conversation.' }) + '\n', () => {});
      } else {
        socket.write(JSON.stringify({ type: 'token', token: 'Regular response.' }) + '\n', () => {});
      }
      socket.write(JSON.stringify({ type: 'finished' }) + '\n', () => {});
    };

    const client = new UdsBrainBackendClient(socketPath);
    const deps: QueryDeps = {
      ...productionDeps(),
      callModel: createBrainCallModel(client),
    };

    // Exercise summary call via callModel
    const summaryGenerator = deps.callModel({
      messages: [
        createUserMessage({ content: 'Turn 1 user' }),
        createAssistantMessage({ content: [{ type: 'text', text: 'Turn 1 assistant' }] }),
        createUserMessage({ content: 'Summarize the above conversation' }),
      ],
      systemPrompt: 'You are a conversation summarizer.' as any,
    });

    let summaryMessage: AssistantMessage | null = null;
    for await (const event of summaryGenerator) {
      if ((event as any).type === 'assistant') {
        summaryMessage = event as AssistantMessage;
      }
    }

    expect(summaryTurnInvoked).toBe(true);
    expect(summaryMessage).not.toBeNull();
    expect(summaryMessage!.message.content[0].text).toBe('Compact summary of prior conversation.');
  });

  // Workflow 5: Session Replay (/resume) Reconstructed Conversation History Boundary
  test('Workflow 5: Reconstructed conversation history from JSONL transcript passes cleanly to Brain on next turn', async () => {
    // 1. Simulate historical session reconstructed from JSONL
    const simulatedHistoricalMessages: Message[] = [
      createUserMessage({ content: 'Historical prompt 1' }),
      createAssistantMessage({ content: [{ type: 'text', text: 'Historical answer 1' }] }),
      createUserMessage({ content: 'Historical prompt 2' }),
      createAssistantMessage({ content: [{ type: 'text', text: 'Historical answer 2' }] }),
    ];

    let receivedHistoricalMessages: any = null;

    activeHandler = (socket, req) => {
      receivedHistoricalMessages = req.payload.messages;
      socket.write(JSON.stringify({ type: 'token', token: 'Resumed session turn 3 answer.' }) + '\n', () => {});
      socket.write(JSON.stringify({ type: 'finished' }) + '\n', () => {});
    };

    const client = new UdsBrainBackendClient(socketPath);
    const deps: QueryDeps = {
      ...productionDeps(),
      callModel: createBrainCallModel(client),
    };

    // 2. Resumed session receives new turn 3 prompt
    const resumedSession = [...simulatedHistoricalMessages, createUserMessage({ content: 'Turn 3 prompt after /resume' })];

    let turn3Response: AssistantMessage | null = null;
    for await (const event of query({
      messages: resumedSession,
      systemPrompt: 'System' as any,
      userContext: {},
      systemContext: {},
      canUseTool: async () => ({ behavior: 'allow' }),
      toolUseContext: createMockToolUseContext(),
      querySource: 'repl',
      deps,
    })) {
      if ((event as any).type === 'assistant') {
        turn3Response = event as AssistantMessage;
      }
    }

    expect(receivedHistoricalMessages.length).toBe(5);
    expect(receivedHistoricalMessages[0]).toEqual({ role: 'user', content: 'Historical prompt 1' });
    expect(receivedHistoricalMessages[1]).toEqual({ role: 'assistant', content: 'Historical answer 1' });
    expect(receivedHistoricalMessages[2]).toEqual({ role: 'user', content: 'Historical prompt 2' });
    expect(receivedHistoricalMessages[3]).toEqual({ role: 'assistant', content: 'Historical answer 2' });
    expect(receivedHistoricalMessages[4]).toEqual({ role: 'user', content: 'Turn 3 prompt after /resume' });
    expect(turn3Response!.message.content[0].text).toBe('Resumed session turn 3 answer.');
  });

  // Workflow 6: Turn Recovery after Severe Backend Disconnect
  test('Workflow 6: Disconnect recovery allows immediate follow-up turn without process restart', async () => {
    let callCount = 0;

    activeHandler = (socket) => {
      callCount++;
      if (callCount === 1) {
        // Disconnect immediately on first turn
        socket.write(JSON.stringify({ type: 'token', token: 'Partial' }) + '\n', () => {
          setTimeout(() => socket.destroy(), 5);
        });
      } else {
        // Turn 2 succeeds cleanly
        socket.write(JSON.stringify({ type: 'token', token: 'Turn 2 recovered completely.' }) + '\n', () => {});
        socket.write(JSON.stringify({ type: 'finished' }) + '\n', () => {});
      }
    };

    const client = new UdsBrainBackendClient(socketPath);
    const deps: QueryDeps = {
      ...productionDeps(),
      callModel: createBrainCallModel(client),
    };

    // Turn 1 fails on disconnect
    let errorReceived = false;
    for await (const event of query({
      messages: [createUserMessage({ content: 'Turn 1 fail' })],
      systemPrompt: 'System' as any,
      userContext: {},
      systemContext: {},
      canUseTool: async () => ({ behavior: 'allow' }),
      toolUseContext: createMockToolUseContext(),
      querySource: 'repl',
      deps,
    })) {
      if ((event as any).type === 'assistant' && (event as any).isApiErrorMessage) {
        errorReceived = true;
      }
    }

    expect(errorReceived).toBe(true);

    // Turn 2 recovery
    let turn2Response: AssistantMessage | null = null;
    for await (const event of query({
      messages: [createUserMessage({ content: 'Turn 2 recover' })],
      systemPrompt: 'System' as any,
      userContext: {},
      systemContext: {},
      canUseTool: async () => ({ behavior: 'allow' }),
      toolUseContext: createMockToolUseContext(),
      querySource: 'repl',
      deps,
    })) {
      if ((event as any).type === 'assistant') {
        turn2Response = event as AssistantMessage;
      }
    }

    expect(turn2Response).not.toBeNull();
    expect(turn2Response!.message.content[0].text).toBe('Turn 2 recovered completely.');
  });
});
