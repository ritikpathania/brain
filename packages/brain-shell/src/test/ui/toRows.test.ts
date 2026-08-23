import { describe, it, expect } from 'bun:test';
import { turnToRows } from '../../ui/transcript/toRows.js';
import type { BrainTurnViewModel, ToolExecutionView } from '../../adapter/BrainViewModels.js';

function vm(patch: Partial<BrainTurnViewModel>): BrainTurnViewModel {
  return {
    id: 'turn_1',
    role: 'assistant',
    content: '',
    status: 'completed',
    durationMs: 100,
    ...patch,
  };
}

describe('turnToRows', () => {
  it('emits thinking, tool, assistant, error rows in stable order', () => {
    const tool: ToolExecutionView = {
      callId: 'call_1',
      toolName: 'read_file',
      input: { path: '/tmp/a.txt' },
      status: 'permission_required',
    };
    const rows = turnToRows(
      vm({
        thinking: { text: 'pondering', isComplete: true, durationMs: 1200 },
        tools: [tool],
        content: '# Answer\nBody text',
        error: 'boom',
      }),
    );
    expect(rows.map((r) => r.kind)).toEqual(['thinking', 'tool', 'assistant', 'error']);
    expect(rows[0]).toMatchObject({ kind: 'thinking', text: 'pondering', durationMs: 1200 });
    expect(rows[1]!.kind === 'tool' && rows[1]!.tool.status).toBe('pending'); // permission_required → pending
    expect(rows[2]!.kind === 'assistant' && rows[2]!.markdown.startsWith('# Answer')).toBe(true);
    expect(rows[3]).toMatchObject({ kind: 'error', text: 'boom' });
    expect(rows.every((r) => r.id.startsWith('turn_1:'))).toBe(true);
  });

  it('omits empty content, absent sections, and memory provenance silently', () => {
    const rows = turnToRows(vm({ memories: [{ nodeId: 'n1', label: 'L', score: 1, source: 's' }] }));
    expect(rows).toEqual([]);
  });
});
