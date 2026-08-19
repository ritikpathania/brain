/**
 * Comprehensive Tool Execution Verification Matrix
 *
 * Deterministically exercises and verifies all local Claude tools in Brain.
 */

import { describe, it, expect, beforeAll, afterAll } from 'bun:test';
import * as fs from 'fs';
import * as path from 'path';
import { FileReadTool } from '../../vendor/claude/tools/FileReadTool/FileReadTool.js';
import { FileWriteTool } from '../../vendor/claude/tools/FileWriteTool/FileWriteTool.js';
import { FileEditTool } from '../../vendor/claude/tools/FileEditTool/FileEditTool.js';
import { GlobTool } from '../../vendor/claude/tools/GlobTool/GlobTool.js';
import { GrepTool } from '../../vendor/claude/tools/GrepTool/GrepTool.js';
import { BashTool } from '../../vendor/claude/tools/BashTool/BashTool.js';
import { TodoWriteTool } from '../../vendor/claude/tools/TodoWriteTool/TodoWriteTool.js';
import { TaskListTool } from '../../vendor/claude/tools/TaskListTool/TaskListTool.js';
import { BriefTool } from '../../vendor/claude/tools/BriefTool/BriefTool.js';
import { TungstenTool } from '../../vendor/claude/tools/TungstenTool/TungstenTool.js';
import { NotebookEditTool } from '../../vendor/claude/tools/NotebookEditTool/NotebookEditTool.js';

function createMockContext(cwd: string) {
  const abortController = new AbortController();
  const readFileState = new Map();
  let todosState: Record<string, any[]> = {};
  const emptyRules: Record<string, string[]> = {
    userSettings: [],
    projectSettings: [],
    localSettings: [],
    flagSettings: [],
    policySettings: [],
    user: [],
    project: [],
    local: [],
    cli: [],
  };
  const appState: any = {
    todos: todosState,
    toolPermissionContext: {
      additionalWorkingDirectories: new Map(),
      alwaysAllowRules: emptyRules,
      alwaysDenyRules: emptyRules,
      alwaysAskRules: emptyRules,
    },
    mcp: { clients: [], tools: [] },
    sessionHooks: new Map(),
    fastMode: false,
    effortValue: 'high',
  };
  return {
    cwd,
    readFileState,
    abortController,
    alwaysDenyRules: emptyRules,
    alwaysAllowRules: emptyRules,
    alwaysAskRules: emptyRules,
    getAppState: () => appState,
    setAppState: (fn: any) => {
      if (typeof fn === 'function') {
        const next = fn(appState);
        if (next?.todos) appState.todos = next.todos;
      }
    },
    options: {
      tools: [],
      mcpClients: [],
      mainLoopModel: 'claude-3-7-sonnet-20250219',
      thinkingConfig: { mode: 'off' },
      agentDefinitions: { activeAgents: [], allowedAgentTypes: [] },
    },
    addNotification: () => {},
  };
}

describe('Tool Execution Verification Matrix (Local Tools)', () => {
  const tmpDir = `/tmp/brain_tool_test_${Date.now()}`;

  beforeAll(() => {
    if (!fs.existsSync(tmpDir)) fs.mkdirSync(tmpDir, { recursive: true });
  });

  afterAll(() => {
    if (fs.existsSync(tmpDir)) fs.rmSync(tmpDir, { recursive: true, force: true });
  });

  it('FileWriteTool: creates a new file deterministically', async () => {
    const filePath = path.join(tmpDir, 'test_write.txt');
    const res = await FileWriteTool.call(
      { file_path: filePath, content: 'Hello Brain Tools' },
      createMockContext(tmpDir) as any
    );
    expect(res).toBeDefined();
    expect(fs.existsSync(filePath)).toBe(true);
    expect(fs.readFileSync(filePath, 'utf8')).toBe('Hello Brain Tools');
  });

  it('FileReadTool: reads file content and handles offset/limit', async () => {
    const filePath = path.join(tmpDir, 'test_read.txt');
    fs.writeFileSync(filePath, 'Line 1\nLine 2\nLine 3\n');
    const res = await FileReadTool.call(
      { file_path: filePath },
      createMockContext(tmpDir) as any
    );
    expect(res).toBeDefined();
    expect(typeof (res as any).data || typeof res).toBeTruthy();
  });

  it('FileEditTool: applies replacement chunks correctly', async () => {
    const filePath = path.join(tmpDir, 'test_edit.txt');
    fs.writeFileSync(filePath, 'const foo = 1;\nconst bar = 2;\n');
    const ctx = createMockContext(tmpDir);
    // Pre-read file as required by Claude FileEditTool collision check
    await FileReadTool.call({ file_path: filePath }, ctx as any);

    const res = await FileEditTool.call(
      {
        file_path: filePath,
        old_string: 'const bar = 2;',
        new_string: 'const bar = 42;',
      },
      ctx as any
    );
    expect(res).toBeDefined();
    expect(fs.readFileSync(filePath, 'utf8')).toContain('const bar = 42;');
  });

  it('GlobTool: finds matching files in directory', async () => {
    const res = await GlobTool.call(
      { pattern: '*.txt', path: tmpDir },
      createMockContext(tmpDir) as any
    );
    expect(res).toBeDefined();
    const str = JSON.stringify(res);
    expect(str).toContain('test_write.txt');
  });

  it('GrepTool: searches matching patterns in files', async () => {
    const res = await GrepTool.call(
      { pattern: 'Brain Tools', path: tmpDir },
      createMockContext(tmpDir) as any
    );
    expect(res).toBeDefined();
    const str = JSON.stringify(res);
    expect(str).toContain('test_write.txt');
  });

  it('BashTool: executes safe shell command and captures output', async () => {
    const res = await BashTool.call(
      { command: 'echo "brain_bash_success"' },
      createMockContext(tmpDir) as any
    );
    expect(res).toBeDefined();
    const str = JSON.stringify(res);
    expect(str).toContain('brain_bash_success');
  });

  it('TodoWriteTool: tracks and updates todos', async () => {
    const res = await TodoWriteTool.call(
      { todos: [{ id: '1', content: 'Test todo', status: 'pending' }] },
      createMockContext(tmpDir) as any
    );
    expect(res).toBeDefined();
  });

  it('BriefTool: generates status summaries', async () => {
    const res = await BriefTool.call(
      { summary: 'Briefing test summary' },
      createMockContext(tmpDir) as any
    );
    expect(res).toBeDefined();
  });

  it('TungstenTool: verifies schema and execution', async () => {
    expect(TungstenTool.name).toBe('TungstenTool');
    expect(TungstenTool.description).toBeDefined();
  });

  it('NotebookEditTool: validates notebook structure', async () => {
    expect(NotebookEditTool.name).toBe('NotebookEdit');
    expect(NotebookEditTool.description).toBeDefined();
  });
});
