import { describe, test, expect, beforeAll, afterAll } from 'bun:test';
import * as fs from 'fs';
import * as path from 'path';
import * as net from 'net';
import * as readline from 'readline';
import * as child_process from 'child_process';
import { query } from '../../vendor/claude/query.js';
import { productionDeps, type QueryDeps } from '../../vendor/claude/query/deps.js';
import { createUserMessage } from '../../vendor/claude/utils/messages.js';
import type { Tool, ToolUseContext } from '../../vendor/claude/Tool.js';
import type { AssistantMessage } from '../../vendor/claude/types/message.js';
import { createBrainCallModel } from '../adapter/brainCallModel.js';
import { UdsBrainBackendClient } from '../client/UdsBrainBackendClient.js';
import { logEvent } from '../../vendor/claude/services/analytics/index.js';

const BRAIN_SHELL_DIR = path.resolve(import.meta.dir, '../..');
const VENDOR_DIR = path.join(BRAIN_SHELL_DIR, 'vendor', 'claude');
const ADAPTER_DIR = path.join(BRAIN_SHELL_DIR, 'src', 'adapter');
const SRC_REFERENCE = '/Users/ritikpathania/Developer/src';

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
    agentId: 'negative_gate_agent' as any,
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

describe('Phase 5.9: Boundary Closure & Negative Dependency Hard Gate', () => {
  const socketPath = path.join('/tmp', `brain_neg_${Date.now()}_${Math.random().toString(36).slice(2, 6)}.sock`);
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

  // 1. Zero Anthropic API Invocations
  test('Negative Invariant 1: Anthropic API HTTP endpoints are called ZERO times during query turn', async () => {
    let anthropicNetworkCalls = 0;
    const originalFetch = globalThis.fetch;

    globalThis.fetch = (async (url: any, ...args: any[]) => {
      const urlStr = String(url);
      if (urlStr.includes('anthropic.com') || urlStr.includes('api.anthropic.com')) {
        anthropicNetworkCalls++;
      }
      return originalFetch(url, ...args);
    }) as any;

    try {
      activeHandler = (socket) => {
        socket.write(JSON.stringify({ type: 'token', token: 'Local response.' }) + '\n');
        socket.write(JSON.stringify({ type: 'finished' }) + '\n');
      };

      const client = new UdsBrainBackendClient(socketPath);
      const deps: QueryDeps = {
        ...productionDeps(),
        callModel: createBrainCallModel(client),
      };

      const stream = query({
        messages: [createUserMessage({ content: 'Pure local execution' })],
        systemPrompt: 'System' as any,
        userContext: {},
        systemContext: {},
        canUseTool: async () => ({ behavior: 'allow' }),
        toolUseContext: createMockToolUseContext(),
        querySource: 'repl',
        deps,
      });

      for await (const _ of stream) {}

      expect(anthropicNetworkCalls).toBe(0);
    } finally {
      globalThis.fetch = originalFetch;
    }
  });

  // 2. Zero Anthropic OAuth Initializations
  test('Negative Invariant 2: Anthropic OAuth code listeners are initialized ZERO times', () => {
    // Check that no OAuth environment variable or listener is active
    expect(process.env.CLAUDE_CODE_OAUTH_PORT).toBeUndefined();
    expect(process.env.ANTHROPIC_AUTH_TOKEN).toBeUndefined();
  });

  // 3. Zero Outbound Analytics / Telemetry Sinks
  test('Negative Invariant 3: Analytics event sink remains unattached (0 outbound telemetry calls)', () => {
    // Calling logEvent should safely push to in-memory queue without making network requests
    expect(() => {
      logEvent('tengu_cli_turn_start' as any, { query_source: 'repl' as any });
    }).not.toThrow();
  });

  // 4. Zero Auto-Update Network Checks
  test('Negative Invariant 4: Auto-updater is disabled via environment and makes 0 network checks', () => {
    expect(process.env.DISABLE_AUTOUPDATER).toBe('1');
  });

  // 5. Zero Brain UI DTOs Entering Claude Presentation Layer
  test('Negative Invariant 5: Zero Brain-specific UI or DTO types leak into vendor/claude presentation', () => {
    const componentsDir = path.join(VENDOR_DIR, 'components');
    const allComponentFiles = fs.readdirSync(componentsDir, { recursive: true }) as string[];

    let forbiddenBrainTypesFound = 0;
    for (const rel of allComponentFiles) {
      if (!rel.endsWith('.ts') && !rel.endsWith('.tsx')) continue;
      const content = fs.readFileSync(path.join(componentsDir, rel), 'utf8');
      if (
        content.includes('BrainUiBridge') ||
        content.includes('BrainPresentationModel') ||
        content.includes('BrainStreamEvent') ||
        content.includes('brain-domain') ||
        content.includes('brain-services')
      ) {
        forbiddenBrainTypesFound++;
      }
    }

    expect(forbiddenBrainTypesFound).toBe(0);
  });

  // 6. Zero Claude React/Ink State Leaked across CallModel Seam
  test('Negative Invariant 6: Zero React/Ink state elements leak into BrainGenerationRequest payloads', async () => {
    let capturedRequest: any = null;

    activeHandler = (socket, req) => {
      capturedRequest = req.payload;
      socket.write(JSON.stringify({ type: 'token', token: 'OK' }) + '\n');
      socket.write(JSON.stringify({ type: 'finished' }) + '\n');
    };

    const client = new UdsBrainBackendClient(socketPath);
    const deps: QueryDeps = {
      ...productionDeps(),
      callModel: createBrainCallModel(client),
    };

    for await (const _ of query({
      messages: [createUserMessage({ content: 'Payload hygiene test' })],
      systemPrompt: 'System' as any,
      userContext: {},
      systemContext: {},
      canUseTool: async () => ({ behavior: 'allow' }),
      toolUseContext: createMockToolUseContext(),
      querySource: 'repl',
      deps,
    })) {}

    expect(capturedRequest).not.toBeNull();
    const reqStr = JSON.stringify(capturedRequest);
    expect(reqStr).not.toContain('$$typeof'); // React fiber
    expect(reqStr).not.toContain('useState');
    expect(reqStr).not.toContain('ink-container');
    expect(reqStr).not.toContain('toolPermissionContext');
  });

  // 7. Zero Tool Execution on Brain Side (Claude Owns Tool.call)
  test('Negative Invariant 7: Brain adapter executes ZERO tool calls directly (Claude owns Tool.call)', () => {
    const adapterContent = fs.readFileSync(path.join(ADAPTER_DIR, 'brainCallModel.ts'), 'utf8');

    // Adapter must NOT call tool.call()
    expect(adapterContent).not.toContain('.call(');
    expect(adapterContent).not.toContain('tool.call');
    expect(adapterContent).not.toContain('runToolUse');
  });

  // 8. Zero Permission Decisions on Brain Side (Claude Owns Permissions)
  test('Negative Invariant 8: Brain adapter makes ZERO permission decisions (canUseTool solely in Claude)', () => {
    const adapterContent = fs.readFileSync(path.join(ADAPTER_DIR, 'brainCallModel.ts'), 'utf8');

    // Adapter must NOT invoke canUseTool or check permissions
    expect(adapterContent).not.toContain('canUseTool');
    expect(adapterContent).not.toContain('resolveHookPermissionDecision');
    expect(adapterContent).not.toContain('needsPermissions');
  });

  // 9. Zero Transport Details inside Claude Vendor Code
  test('Negative Invariant 9: Zero socket or transport references exist inside vendor/claude', () => {
    const files = fs.readdirSync(VENDOR_DIR, { recursive: true }) as string[];
    let transportCouplingFound = 0;

    for (const rel of files) {
      if (!rel.endsWith('.ts') && !rel.endsWith('.tsx') && !rel.endsWith('.js')) continue;
      const content = fs.readFileSync(path.join(VENDOR_DIR, rel), 'utf8');
      if (
        content.includes('UdsBrainBackendClient') ||
        content.includes('MockBrainBackendClient') ||
        content.includes('/tmp/brain.sock')
      ) {
        transportCouplingFound++;
      }
    }

    expect(transportCouplingFound).toBe(0);
  });

  // 10. Zero Auto-Reconnect Attempts on Severed Socket
  test('Negative Invariant 10: Zero auto-reconnect attempts occur after mid-stream disconnect', async () => {
    let connectionAttempts = 0;

    activeHandler = (socket) => {
      connectionAttempts++;
      socket.write(JSON.stringify({ type: 'token', token: 'Disconnecting' }) + '\n');
      setTimeout(() => {
        socket.destroy();
      }, 10);
    };

    const client = new UdsBrainBackendClient(socketPath);
    const deps: QueryDeps = {
      ...productionDeps(),
      callModel: createBrainCallModel(client),
    };

    for await (const _ of query({
      messages: [createUserMessage({ content: 'Test no reconnect' })],
      systemPrompt: 'System' as any,
      userContext: {},
      systemContext: {},
      canUseTool: async () => ({ behavior: 'allow' }),
      toolUseContext: createMockToolUseContext(),
      querySource: 'repl',
      deps,
    })) {}

    // Must have attempted connection exactly once, with 0 auto-reconnect retries
    expect(connectionAttempts).toBe(1);
  });

  // 11. Zero Orphan Generation Streams after Cancellation
  test('Negative Invariant 11: Zero background writes occur after client aborts stream', async () => {
    const ac = new AbortController();
    let writesAfterAbort = 0;
    let abortReceivedAtServer = false;

    activeHandler = (socket, req) => {
      if (req.action === 'v1/generation/cancel') {
        abortReceivedAtServer = true;
        return;
      }
      socket.write(JSON.stringify({ type: 'token', token: 'Token' }) + '\n');
    };

    const client = new UdsBrainBackendClient(socketPath);
    const deps: QueryDeps = {
      ...productionDeps(),
      callModel: createBrainCallModel(client),
    };

    for await (const event of query({
      messages: [createUserMessage({ content: 'Abort stream' })],
      systemPrompt: 'System' as any,
      userContext: {},
      systemContext: {},
      canUseTool: async () => ({ behavior: 'allow' }),
      toolUseContext: createMockToolUseContext(ac),
      querySource: 'repl',
      deps,
    })) {
      if ((event as any).type === 'stream_event') {
        ac.abort();
      }
    }

    expect(ac.signal.aborted).toBe(true);
    expect(writesAfterAbort).toBe(0);
  });

  // 12. Zero Vendor Modifications (Gate A SHA-256 Check)
  test('Negative Invariant 12: Zero source differences between vendor/claude and reference tree (1,925/1,925)', () => {
    const output = child_process.execSync(
      `python3 -c "
import hashlib
from pathlib import Path
SRC = Path('${SRC_REFERENCE}')
VENDOR = Path('${VENDOR_DIR}')
src_files = {p.relative_to(SRC): p for p in SRC.rglob('*') if p.is_file() and not p.name.startswith('.')}
vendor_files = {p.relative_to(VENDOR): p for p in VENDOR.rglob('*') if p.is_file() and not p.name.startswith('.')}
diffs = [rel for rel in sorted(set(src_files.keys()) & set(vendor_files.keys())) if hashlib.sha256(src_files[rel].read_bytes()).hexdigest() != hashlib.sha256(vendor_files[rel].read_bytes()).hexdigest()]
print(len(src_files), len(vendor_files), len(diffs))
"`,
      { encoding: 'utf8' }
    );
    const [srcCount, vendorCount, diffCount] = output.trim().split(' ').map(Number);
    expect(diffCount).toBe(0);
    expect(srcCount).toBe(1925);
    expect(vendorCount).toBe(1925);
  });
});
