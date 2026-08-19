import { describe, test, expect } from 'bun:test';
import * as child_process from 'child_process';
import * as path from 'path';

import { query } from '../../../vendor/claude/query.js';
import { productionDeps, type QueryDeps } from '../../../vendor/claude/query/deps.js';
import { createUserMessage } from '../../../vendor/claude/utils/messages.js';
import type { Tool, ToolUseContext } from '../../../vendor/claude/Tool.js';
import type { Message, AssistantMessage } from '../../../vendor/claude/types/message.js';
import { MockBrainBackendClient, type BrainBackendClient, type BrainGenerationRequest, type BrainStreamChunk } from '../../client/BrainBackendClient.js';
import { createBrainCallModel } from '../../adapter/brainCallModel.js';

// Ensure mock OAuth token is set
process.env.CLAUDE_CODE_OAUTH_TOKEN = process.env.CLAUDE_CODE_OAUTH_TOKEN || 'test-oauth-token-for-runtime';
delete process.env.ANTHROPIC_API_KEY;

const TEST_DIR = import.meta.dir;
const BRAIN_SHELL_DIR = path.resolve(TEST_DIR, '..', '..', '..');
const RUNTIME_RUNNER = path.join(TEST_DIR, 'runtimeRunner.py');

const CANONICAL_VIEWPORTS = [
  { name: '80x24 (Standard Compact)', cols: 80, rows: 24 },
  { name: '100x26 (Medium Desktop)', cols: 100, rows: 26 },
  { name: '120x30 (Widescreen Terminal)', cols: 120, rows: 30 },
  { name: '182x53 (Fullscreen Display)', cols: 182, rows: 53 },
];

class AdversarialRuntimeBrainClient implements BrainBackendClient {
  public activeStreams = 0;
  public totalStreamsStarted = 0;
  public totalStreamsCompleted = 0;
  public totalStreamsAborted = 0;
  public cancelFrameCount = 0;
  public orphanTokensEmitted = 0;

  constructor(
    private chunkGenerator?: (req: BrainGenerationRequest, client: AdversarialRuntimeBrainClient) => AsyncIterable<BrainStreamChunk>
  ) {}

  async *streamText(request: BrainGenerationRequest): AsyncIterable<BrainStreamChunk> {
    this.activeStreams++;
    this.totalStreamsStarted++;

    let observedAbort = false;
    request.signal?.addEventListener('abort', () => {
      observedAbort = true;
      this.totalStreamsAborted++;
      this.cancelFrameCount++;
    });

    try {
      if (this.chunkGenerator) {
        for await (const chunk of this.chunkGenerator(request, this)) {
          if (request.signal?.aborted || observedAbort) {
            this.orphanTokensEmitted++;
            break;
          }
          yield chunk;
          await new Promise((r) => setTimeout(r, 1));
        }
      } else {
        yield { type: 'finished' };
      }
    } finally {
      this.activeStreams--;
      this.totalStreamsCompleted++;
    }
  }
}

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
    agentId: 'runtime_test_agent' as any,
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

function createDummyTool(name: string, executeResult: (input: any) => Promise<any> = async () => 'Tool Output OK'): Tool {
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

describe('Phase 7B Wave 3: Runtime Execution State Machine Contracts (States 12–13b)', () => {

  // ==========================================================================
  // STATE 12: ACTIVE_STREAMING_TURN & TEMPORAL EVENT PROGRESSION
  // ==========================================================================
  describe('State 12: ACTIVE_STREAMING_TURN Contract (12a Thinking + 12b Text)', () => {
    test('Dimension 1–5: Explicit event ordering invariant (thinking_start -> thinking_delta* -> thinking_end -> text_start -> text_delta* -> stream_end)', async () => {
      const observedEvents: string[] = [];

      const brainClient = new MockBrainBackendClient(async function* () {
        observedEvents.push('backend_generation_start');
        yield { type: 'thinking', thinking: 'Step 1: Analyzing requirements' };
        yield { type: 'thinking', thinking: 'Step 2: Synthesizing response' };
        yield { type: 'token', token: 'Here is the verified ' };
        yield { type: 'token', token: 'streaming response.' };
        yield { type: 'finished' };
        observedEvents.push('backend_stream_end');
      });

      const deps: QueryDeps = {
        ...productionDeps(),
        callModel: createBrainCallModel(brainClient),
      };

      const stream = query({
        messages: [createUserMessage({ content: 'Explain streaming architecture' })],
        systemPrompt: 'System' as any,
        userContext: {},
        systemContext: {},
        canUseTool: async () => ({ behavior: 'allow' }),
        toolUseContext: createMockToolUseContext(),
        querySource: 'repl',
        deps,
      });

      let turnCompleteSeen = false;
      for await (const event of stream) {
        if ((event as any).type === 'stream_event') {
          const se = (event as any).event;
          if (se.type === 'content_block_start') {
            if (se.content_block?.type === 'thinking') observedEvents.push('thinking_start');
            else if (se.content_block?.type === 'text') observedEvents.push('text_start');
          } else if (se.type === 'content_block_delta') {
            if (se.delta?.type === 'thinking_delta') observedEvents.push('thinking_delta');
            else if (se.delta?.type === 'text_delta') observedEvents.push('text_delta');
          } else if (se.type === 'content_block_stop') {
            if (observedEvents.includes('thinking_start') && !observedEvents.includes('thinking_end')) {
              observedEvents.push('thinking_end');
            }
          } else if (se.type === 'message_stop') {
            observedEvents.push('stream_end');
          }
        } else if ((event as any).type === 'assistant') {
          observedEvents.push('turn_complete');
          turnCompleteSeen = true;
        }
      }

      expect(turnCompleteSeen).toBe(true);

      const thinkingStartIndex = observedEvents.indexOf('thinking_start');
      const thinkingEndIndex = observedEvents.indexOf('thinking_end');
      const textStartIndex = observedEvents.indexOf('text_start');
      const streamEndIndex = observedEvents.indexOf('stream_end');
      const turnCompleteIndex = observedEvents.indexOf('turn_complete');

      expect(thinkingStartIndex).toBeGreaterThanOrEqual(0);
      expect(thinkingEndIndex).toBeGreaterThan(thinkingStartIndex);
      expect(textStartIndex).toBeGreaterThan(thinkingEndIndex);
      expect(streamEndIndex).toBeGreaterThan(textStartIndex);
      expect(turnCompleteIndex).toBeGreaterThan(streamEndIndex);
    });

    test('Dimension 1–5: Zero-loss boundary assertion across thinking -> text and text -> stream_end', async () => {
      const thinkingTokens = ['First thought', 'Second thought', 'Third thought'];
      const textTokens = ['Token Alpha ', 'Token Beta ', 'Token Gamma.'];

      const brainClient = new MockBrainBackendClient(async function* () {
        for (const t of thinkingTokens) yield { type: 'thinking', thinking: t };
        for (const t of textTokens) yield { type: 'token', token: t };
        yield { type: 'finished' };
      });

      const deps: QueryDeps = {
        ...productionDeps(),
        callModel: createBrainCallModel(brainClient),
      };

      const stream = query({
        messages: [createUserMessage({ content: 'Test zero-loss tokens' })],
        systemPrompt: 'System' as any,
        userContext: {},
        systemContext: {},
        canUseTool: async () => ({ behavior: 'allow' }),
        toolUseContext: createMockToolUseContext(),
        querySource: 'repl',
        deps,
      });

      const collectedThinking: string[] = [];
      const collectedText: string[] = [];
      let finalMsg: AssistantMessage | null = null;

      for await (const event of stream) {
        if ((event as any).type === 'stream_event') {
          const delta = (event as any).event?.delta;
          if (delta?.type === 'thinking_delta') collectedThinking.push(delta.thinking);
          if (delta?.type === 'text_delta') collectedText.push(delta.text);
        } else if ((event as any).type === 'assistant') {
          finalMsg = event as AssistantMessage;
        }
      }

      // Assert 0 tokens lost and 0 tokens duplicated
      expect(collectedThinking).toEqual(thinkingTokens);
      expect(collectedText).toEqual(textTokens);
      expect(finalMsg).not.toBeNull();
    });

    for (const vp of CANONICAL_VIEWPORTS) {
      test(`Dimension 4 (Layer 1): Multi-viewport PTY streaming turn rendering across ${vp.name}`, () => {
        const output = child_process.execSync(
          `python3 ${RUNTIME_RUNNER} streaming_turn "${BRAIN_SHELL_DIR}" ${vp.cols} ${vp.rows}`,
          { encoding: 'utf8', timeout: 15000 }
        );

        const EXPECTED_VERSION = process.env.CLAUDE_VERSION || (globalThis as any).MACRO?.VERSION || '2.1.235';
        expect(output).toContain(`Claude Code v${EXPECTED_VERSION}`);
        expect(output).toContain('Sonnet 4.6');
      }, 15000);
    }
  });

  // ==========================================================================
  // STATE 13: TOOL_EXECUTION & CLAUDE-OWNED LIFECYCLE
  // ==========================================================================
  describe('State 13: TOOL_EXECUTION Contract (Claude Tool.call Ownership)', () => {
    test('Dimension 1–5: Brain emits tool_use, Claude runtime executes Tool.call, results reach synthesis', async () => {
      let claudeToolExecuted = false;
      const testTool = createDummyTool('test_database_query', async (input) => {
        claudeToolExecuted = true;
        return `Executed SQL: ${input.sql}`;
      });

      let brainTurnCount = 0;
      const brainClient = new MockBrainBackendClient(async function* (req: BrainGenerationRequest) {
        brainTurnCount++;
        if (brainTurnCount === 1) {
          // Turn 1: Brain emits tool_use
          yield {
            type: 'tool_use',
            toolUse: {
              id: 'call_sql_101',
              name: 'test_database_query',
              input: { sql: 'SELECT * FROM nodes' },
            },
          };
          yield { type: 'finished' };
        } else {
          // Turn 2: Brain receives tool result and produces final synthesis
          yield { type: 'token', token: 'Found 42 records matching query.' };
          yield { type: 'finished' };
        }
      });

      const deps: QueryDeps = {
        ...productionDeps(),
        callModel: createBrainCallModel(brainClient),
      };

      const stream = query({
        messages: [createUserMessage({ content: 'Query database' })],
        systemPrompt: 'System' as any,
        userContext: {},
        systemContext: {},
        canUseTool: async () => ({ behavior: 'allow' }),
        toolUseContext: createMockToolUseContext(new AbortController(), [testTool]),
        querySource: 'repl',
        deps,
      });

      let finalSynthesis = '';
      for await (const event of stream) {
        if ((event as any).type === 'stream_event') {
          const delta = (event as any).event?.delta;
          if (delta?.type === 'text_delta') finalSynthesis += delta.text;
        }
      }

      // Assert Claude runtime executed the tool directly (Brain did not execute tool)
      expect(claudeToolExecuted).toBe(true);
      expect(brainTurnCount).toBe(2);
      expect(finalSynthesis).toContain('Found 42 records matching query.');
    });
  });

  // ==========================================================================
  // STATE 13b: CANCELLATION_ABORT (5-POINT ADVERSARIAL MATRIX)
  // ==========================================================================
  describe('State 13b: CANCELLATION_ABORT Adversarial Matrix (5 Points of Interruption)', () => {
    test('Point 1: Abort before first token (Immediate abort on dispatch)', async () => {
      const client = new AdversarialRuntimeBrainClient(async function* () {
        yield { type: 'token', token: 'Ghost token' };
      });

      const abortController = new AbortController();
      abortController.abort(); // Pre-abort

      const deps: QueryDeps = {
        ...productionDeps(),
        callModel: createBrainCallModel(client),
      };

      const stream = query({
        messages: [createUserMessage({ content: 'Immediate abort' })],
        systemPrompt: 'System' as any,
        userContext: {},
        systemContext: {},
        canUseTool: async () => ({ behavior: 'allow' }),
        toolUseContext: createMockToolUseContext(abortController),
        querySource: 'repl',
        deps,
      });

      for await (const event of stream) {}

      expect(client.activeStreams).toBe(0);
      expect(client.orphanTokensEmitted).toBe(0);
    });

    test('Point 2: Abort mid-thinking block', async () => {
      const abortController = new AbortController();
      const client = new AdversarialRuntimeBrainClient(async function* () {
        yield { type: 'thinking', thinking: 'Thinking 1' };
        yield { type: 'thinking', thinking: 'Thinking 2' };
        yield { type: 'thinking', thinking: 'Ghost Thinking 3' };
      });

      const deps: QueryDeps = {
        ...productionDeps(),
        callModel: createBrainCallModel(client),
      };

      const stream = query({
        messages: [createUserMessage({ content: 'Abort during thinking' })],
        systemPrompt: 'System' as any,
        userContext: {},
        systemContext: {},
        canUseTool: async () => ({ behavior: 'allow' }),
        toolUseContext: createMockToolUseContext(abortController),
        querySource: 'repl',
        deps,
      });

      let thinkingEvents = 0;
      for await (const event of stream) {
        if ((event as any).type === 'stream_event' && (event as any).event?.delta?.type === 'thinking_delta') {
          thinkingEvents++;
          if (thinkingEvents === 1) {
            abortController.abort();
          }
        }
      }

      expect(client.totalStreamsAborted).toBe(1);
      expect(client.cancelFrameCount).toBe(1);
      expect(client.activeStreams).toBe(0);
    });

    test('Point 3: Abort mid-text streaming', async () => {
      const abortController = new AbortController();
      const client = new AdversarialRuntimeBrainClient(async function* () {
        for (let i = 0; i < 20; i++) {
          yield { type: 'token', token: `Token_${i} ` };
        }
      });

      const deps: QueryDeps = {
        ...productionDeps(),
        callModel: createBrainCallModel(client),
      };

      const stream = query({
        messages: [createUserMessage({ content: 'Abort during text' })],
        systemPrompt: 'System' as any,
        userContext: {},
        systemContext: {},
        canUseTool: async () => ({ behavior: 'allow' }),
        toolUseContext: createMockToolUseContext(abortController),
        querySource: 'repl',
        deps,
      });

      let textEvents = 0;
      for await (const event of stream) {
        if ((event as any).type === 'stream_event' && (event as any).event?.delta?.type === 'text_delta') {
          textEvents++;
          if (textEvents === 2) {
            abortController.abort();
          }
        }
      }

      expect(client.totalStreamsAborted).toBe(1);
      expect(client.cancelFrameCount).toBe(1);
      expect(client.activeStreams).toBe(0);
    });

    test('Point 4: Abort mid-tool execution', async () => {
      let toolExecuted = false;
      const abortController = new AbortController();

      const slowTool = createDummyTool('slow_tool', async () => {
        toolExecuted = true;
        abortController.abort();
        return 'Done';
      });

      const client = new AdversarialRuntimeBrainClient(async function* () {
        yield {
          type: 'tool_use',
          toolUse: { id: 'call_slow', name: 'slow_tool', input: {} },
        };
        yield { type: 'finished' };
      });

      const deps: QueryDeps = {
        ...productionDeps(),
        callModel: createBrainCallModel(client),
      };

      const stream = query({
        messages: [createUserMessage({ content: 'Execute slow tool' })],
        systemPrompt: 'System' as any,
        userContext: {},
        systemContext: {},
        canUseTool: async () => ({ behavior: 'allow' }),
        toolUseContext: createMockToolUseContext(abortController, [slowTool]),
        querySource: 'repl',
        deps,
      });

      for await (const event of stream) {}

      expect(toolExecuted).toBe(true);
      expect(client.activeStreams).toBe(0);
    });

    test('Point 5: Abort post-tool / pre-synthesis & Recovery for subsequent prompt', async () => {
      const abortController1 = new AbortController();
      let callCount = 0;
      const testTool = createDummyTool('fast_tool', async () => 'Tool Output');

      const client1 = new AdversarialRuntimeBrainClient(async function* () {
        callCount++;
        if (callCount === 1) {
          yield {
            type: 'tool_use',
            toolUse: { id: 'call_fast', name: 'fast_tool', input: {} },
          };
          yield { type: 'finished' };
        } else {
          // Abort right as turn 2 starts
          abortController1.abort();
          yield { type: 'token', token: 'Ghost synthesis' };
          yield { type: 'finished' };
        }
      });

      const deps1: QueryDeps = {
        ...productionDeps(),
        callModel: createBrainCallModel(client1),
      };

      const stream1 = query({
        messages: [createUserMessage({ content: 'Run tool and abort synthesis' })],
        systemPrompt: 'System' as any,
        userContext: {},
        systemContext: {},
        canUseTool: async () => ({ behavior: 'allow' }),
        toolUseContext: createMockToolUseContext(abortController1, [testTool]),
        querySource: 'repl',
        deps: deps1,
      });

      for await (const event of stream1) {}
      expect(client1.activeStreams).toBe(0);

      // --- Recovery: Second turn with clean prompt must succeed 100% ---
      const client2 = new AdversarialRuntimeBrainClient(async function* () {
        yield { type: 'token', token: 'Recovery successful!' };
        yield { type: 'finished' };
      });

      const deps2: QueryDeps = {
        ...productionDeps(),
        callModel: createBrainCallModel(client2),
      };

      const stream2 = query({
        messages: [createUserMessage({ content: 'Follow-up query after abort' })],
        systemPrompt: 'System' as any,
        userContext: {},
        systemContext: {},
        canUseTool: async () => ({ behavior: 'allow' }),
        toolUseContext: createMockToolUseContext(new AbortController()),
        querySource: 'repl',
        deps: deps2,
      });

      let recoveredText = '';
      for await (const event of stream2) {
        if ((event as any).type === 'stream_event') {
          const delta = (event as any).event?.delta;
          if (delta?.type === 'text_delta') recoveredText += delta.text;
        }
      }

      expect(recoveredText).toContain('Recovery successful!');
    });
  });
});
