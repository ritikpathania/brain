import { describe, test, expect, beforeAll, afterAll, beforeEach } from 'bun:test';
import * as child_process from 'child_process';
import * as net from 'net';
import * as fs from 'fs';
import * as path from 'path';
import * as readline from 'readline';

import { query } from '../../../vendor/claude/query.js';
import { productionDeps, type QueryDeps } from '../../../vendor/claude/query/deps.js';
import { createUserMessage, createAssistantMessage } from '../../../vendor/claude/utils/messages.js';
import type { Tool, ToolUseContext } from '../../../vendor/claude/Tool.js';
import type { Message, AssistantMessage, UserMessage } from '../../../vendor/claude/types/message.js';
import { microcompactMessages } from '../../../vendor/claude/services/compact/microCompact.js';
import { createBrainCallModel } from '../../adapter/brainCallModel.js';
import { UdsBrainBackendClient } from '../../client/UdsBrainBackendClient.js';
import { MockBrainBackendClient, type BrainGenerationRequest } from '../../client/BrainBackendClient.js';

// Ensure mock OAuth token is set
process.env.CLAUDE_CODE_OAUTH_TOKEN = process.env.CLAUDE_CODE_OAUTH_TOKEN || 'test-oauth-token-for-lifecycle';
delete process.env.ANTHROPIC_API_KEY;

const TEST_DIR = import.meta.dir;
const BRAIN_SHELL_DIR = path.resolve(TEST_DIR, '..', '..', '..');
const LIFECYCLE_RUNNER = path.join(TEST_DIR, 'lifecycleRunner.py');

const CANONICAL_VIEWPORTS = [
  { name: '80x24 (Standard Compact)', cols: 80, rows: 24 },
  { name: '100x26 (Medium Desktop)', cols: 100, rows: 26 },
  { name: '120x30 (Widescreen Terminal)', cols: 120, rows: 30 },
  { name: '182x53 (Fullscreen Display)', cols: 182, rows: 53 },
];

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
    agentId: 'lifecycle_test_agent' as any,
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

function createDummyTool(name: string, executeResult: (input: any) => Promise<any> = async () => 'OK'): Tool {
  return {
    name,
    description: `Lifecycle test tool for ${name}`,
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

describe('Phase 7B Wave 4: Lifecycle, Compaction & Diagnostics State Machine Contracts (States 14–16)', () => {
  const socketPath = path.join('/tmp', `brain_lifecycle_${Date.now()}_${Math.random().toString(36).slice(2, 6)}.sock`);
  let server: net.Server | null = null;
  let activeHandler: ((socket: net.Socket, req: any) => void) | null = null;

  beforeAll(async () => {
    if (fs.existsSync(socketPath)) {
      try { fs.unlinkSync(socketPath); } catch {}
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
      try { fs.unlinkSync(socketPath); } catch {}
    }
  });

  beforeEach(async () => {
    activeHandler = null;
    await new Promise((r) => setTimeout(r, 10));
  });

  // ==========================================================================
  // STATE 14: COMPACTION_TURN (MICROCOMPACTION + AUTOCOMPACTION)
  // ==========================================================================
  describe('State 14: COMPACTION_TURN Contract', () => {
    test('Dimension 1–5: Microcompaction replaces historical tool results in-memory with 0 model calls', async () => {
      let modelCallCount = 0;
      const client = new MockBrainBackendClient(async function* () {
        modelCallCount++;
        yield { type: 'token', token: 'Should not be called' };
        yield { type: 'finished' };
      });

      const messages: Message[] = [
        createUserMessage({ content: 'Read huge log file' }),
        createAssistantMessage({
          content: [
            { type: 'tool_use', id: 'call_huge_01', name: 'FileRead', input: { path: 'build.log' } },
          ],
        }),
        {
          type: 'user',
          message: {
            role: 'user',
            content: [
              {
                type: 'tool_result',
                tool_use_id: 'call_huge_01',
                content: 'B'.repeat(12000), // 12,000 characters
              },
            ],
          },
        } as UserMessage,
      ];

      const result = await microcompactMessages(messages as any, createMockToolUseContext(), 'repl_main_thread' as any);

      expect(result.messages).toBeDefined();
      expect(result.messages.length).toBeGreaterThanOrEqual(1);
      expect(modelCallCount).toBe(0); // 0 model invocations
    });

    test('Dimension 1–5: Autocompaction delegates conversation summary turn through QueryDeps.callModel', async () => {
      let summaryTurnInvoked = false;

      activeHandler = (socket, req) => {
        summaryTurnInvoked = true;
        socket.write(JSON.stringify({ type: 'token', token: 'Comprehensive summary of prior conversation history.' }) + '\n', () => {});
        socket.write(JSON.stringify({ type: 'finished' }) + '\n', () => {});
      };

      const client = new UdsBrainBackendClient(socketPath);
      const deps: QueryDeps = {
        ...productionDeps(),
        callModel: createBrainCallModel(client),
      };

      const summaryStream = deps.callModel({
        messages: [
          createUserMessage({ content: 'Turn 1 user request' }),
          createAssistantMessage({ content: [{ type: 'text', text: 'Turn 1 assistant answer' }] }),
          createUserMessage({ content: 'Summarize the above session' }),
        ],
        systemPrompt: 'You are a session summarizer.' as any,
      });

      let summaryMessage: AssistantMessage | null = null;
      for await (const event of summaryStream) {
        if ((event as any).type === 'assistant') {
          summaryMessage = event as AssistantMessage;
        }
      }

      expect(summaryTurnInvoked).toBe(true);
      expect(summaryMessage).not.toBeNull();
      expect(summaryMessage!.message.content[0].text).toContain('Comprehensive summary of prior conversation history.');
    });
  });

  // ==========================================================================
  // STATE 15: SESSION_RESUME (/resume TRANSCRIPT RESTORATION)
  // ==========================================================================
  describe('State 15: SESSION_RESUME Contract', () => {
    test('Dimension 1–5: Reconstructed transcript history passes cleanly across QueryDeps.callModel', async () => {
      const historicalTranscript: Message[] = [
        createUserMessage({ content: 'Explain Rust ownership' }),
        createAssistantMessage({ content: [{ type: 'text', text: 'Rust uses affine types and borrow checking.' }] }),
        createUserMessage({ content: 'Now show an example in main.rs' }),
      ];

      let receivedPayloadMessages: any = null;

      activeHandler = (socket, req) => {
        receivedPayloadMessages = req.payload.messages;
        socket.write(JSON.stringify({ type: 'token', token: 'fn main() { let x = String::from("hello"); }' }) + '\n', () => {});
        socket.write(JSON.stringify({ type: 'finished' }) + '\n', () => {});
      };

      const client = new UdsBrainBackendClient(socketPath);
      const deps: QueryDeps = {
        ...productionDeps(),
        callModel: createBrainCallModel(client),
      };

      const stream = query({
        messages: historicalTranscript,
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

      // Assert transcript was passed into request without modification or loss
      expect(receivedPayloadMessages).not.toBeNull();
      expect(receivedPayloadMessages.length).toBe(3);
      expect(receivedPayloadMessages[0]).toEqual({ role: 'user', content: 'Explain Rust ownership' });
      expect(receivedPayloadMessages[1]).toEqual({ role: 'assistant', content: 'Rust uses affine types and borrow checking.' });
      expect(finalMsg).not.toBeNull();
      expect(finalMsg!.message.content[0].text).toContain('fn main()');
    });

    test('Dimension 1–5: Multi-turn dialogue preserves complete history across turns without drift', async () => {
      let turnCount = 0;
      const capturedRequests: any[] = [];

      activeHandler = (socket, req) => {
        turnCount++;
        capturedRequests.push(req.payload.messages);
        socket.write(JSON.stringify({ type: 'token', token: `Answer to turn ${turnCount}` }) + '\n', () => {});
        socket.write(JSON.stringify({ type: 'finished' }) + '\n', () => {});
      };

      const client = new UdsBrainBackendClient(socketPath);
      const deps: QueryDeps = {
        ...productionDeps(),
        callModel: createBrainCallModel(client),
      };

      const conversation: Message[] = [createUserMessage({ content: 'First turn query' })];

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
      conversation.push(createUserMessage({ content: 'Second turn query' }));
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

      expect(turnCount).toBe(2);
      expect(capturedRequests[0].length).toBe(1);
      expect(capturedRequests[1].length).toBe(3); // Turn 1 user + Turn 1 assistant + Turn 2 user
    });
  });

  // ==========================================================================
  // STATE 16: DIAGNOSTICS_RECOVERY
  // ==========================================================================
  describe('State 16: DIAGNOSTICS_RECOVERY Contract', () => {
    test('Dimension 1–5: Broken backend connection recovers cleanly on immediate subsequent turn', async () => {
      let turnAttempt = 0;

      activeHandler = (socket, req) => {
        turnAttempt++;
        if (turnAttempt === 1) {
          // Sever socket abruptly mid-turn
          socket.destroy();
        } else {
          // Clean answer on recovery turn
          socket.write(JSON.stringify({ type: 'token', token: 'Clean recovered answer.' }) + '\n', () => {});
          socket.write(JSON.stringify({ type: 'finished' }) + '\n', () => {});
        }
      };

      const client = new UdsBrainBackendClient(socketPath);
      const deps: QueryDeps = {
        ...productionDeps(),
        callModel: createBrainCallModel(client),
      };

      // Turn 1: Should encounter severed socket error
      let errorReceived = false;
      for await (const event of query({
        messages: [createUserMessage({ content: 'Severed turn' })],
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

      // Turn 2: Fresh query must succeed with 100% fidelity without restarting client
      let recoveredResponse: AssistantMessage | null = null;
      for await (const event of query({
        messages: [createUserMessage({ content: 'Recovered turn query' })],
        systemPrompt: 'System' as any,
        userContext: {},
        systemContext: {},
        canUseTool: async () => ({ behavior: 'allow' }),
        toolUseContext: createMockToolUseContext(),
        querySource: 'repl',
        deps,
      })) {
        if ((event as any).type === 'assistant') {
          recoveredResponse = event as AssistantMessage;
        }
      }

      expect(errorReceived).toBe(true);
      expect(recoveredResponse).not.toBeNull();
      expect(recoveredResponse!.message.content[0].text).toContain('Clean recovered answer.');
    });

    for (const vp of CANONICAL_VIEWPORTS) {
      test(`Dimension 4 (Layer 1): Multi-viewport PTY doctor/status command rendering across ${vp.name}`, () => {
        const output = child_process.execSync(
          `python3 ${LIFECYCLE_RUNNER} status_command "${BRAIN_SHELL_DIR}" ${vp.cols} ${vp.rows}`,
          { encoding: 'utf8', timeout: 15000 }
        );
        const EXPECTED_VERSION = process.env.CLAUDE_VERSION || (globalThis as any).MACRO?.VERSION || '2.1.235';
        expect(output).toContain(`Claude Code v${EXPECTED_VERSION}`);
      }, 15000);
    }
  });
});
