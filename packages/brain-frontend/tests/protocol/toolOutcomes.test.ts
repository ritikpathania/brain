import { expect, test } from 'bun:test';
import { GenerationStreamGuard } from '../../src/brain/generation.js';

const correlation = { sessionId: '01ARZ3NDEKTSV4RRFFQ69G5FAV', generationId: 'g', connectionEpoch: 1 };
function frame(sequence: number, type: string, extra: Record<string, unknown> = {}) {
  return { sequence, type, session_id: correlation.sessionId, generation_id: 'g', ...extra };
}
function pendingTool() {
  const guard = new GenerationStreamGuard(correlation);
  guard.accept(frame(0, 'stream_start'));
  guard.accept(frame(1, 'tool_use', { toolUse: { id: 'call', name: 'bash', input: { command: 'printf ok' } } }));
  guard.accept(frame(2, 'tool_permission_requested', { call_id: 'call', permission_request_id: 'p', tool_name: 'bash', input: { command: 'printf ok' } }));
  return guard;
}

for (const [outcome, status] of [['success', 'succeeded'], ['error', 'failed'], ['timed_out', 'timed_out'], ['cancelled', 'cancelled']] as const) {
  test(`preserves the authoritative ${outcome} tool outcome`, () => {
    const guard = pendingTool();
    const event = guard.accept(frame(3, 'tool_result', { call_id: 'call', permission_request_id: 'p', tool_name: 'bash', outcome, output: 'ok' }));
    expect(event.tool?.status).toBe(status);
    expect(event.tool?.output).toBe('ok');
    expect(() => guard.accept(frame(4, 'tool_result', { call_id: 'call', permission_request_id: 'p', outcome }))).toThrow('tool_correlation');
    guard.accept(frame(4, 'finished', { status: 'completed' }));
    expect(() => guard.accept(frame(5, 'tool_result'))).toThrow('post_terminal');
  });
}

test('tool output is bounded and cannot inject ANSI, OSC or control sequences', () => {
  const event = pendingTool().accept(frame(3, 'tool_result', { call_id: 'call', permission_request_id: 'p', outcome: 'success', output: '\u001b[31mred\u001b[0m\u001b]52;c;clipboard\u0007\u0000' + 'x'.repeat(10_000) }));
  expect(event.tool?.output).not.toContain('\u001b');
  expect(event.tool?.output).not.toContain('clipboard');
  expect(event.tool?.output).not.toContain('\u0000');
  expect(event.tool?.output?.length).toBeLessThanOrEqual(4_096);
  expect(event.tool?.output).toContain('[output truncated]');
});

test('malformed outcomes and mismatched permission/tool names are rejected before settlement', () => {
  const guard = pendingTool();
  expect(() => guard.accept(frame(3, 'tool_result', { call_id: 'call', permission_request_id: 'p', outcome: 'invented' }))).toThrow('tool_correlation');
  expect(() => guard.accept(frame(3, 'tool_result', { call_id: 'call', permission_request_id: 'p', tool_name: 'other', outcome: 'success' }))).toThrow('tool_correlation');
  expect(() => guard.accept(frame(3, 'tool_result', { call_id: 'call', permission_request_id: 'p', outcome: 'success' }), 0)).toThrow('foreign_frame');
  expect(guard.accept(frame(3, 'tool_denied', { call_id: 'call', permission_request_id: 'p', outcome: 'denied' })).tool?.status).toBe('failed');
});

test('permission retains validated inputs internally, without accepting a foreign tool name', () => {
  const guard = new GenerationStreamGuard(correlation);
  guard.accept(frame(0, 'stream_start'));
  guard.accept(frame(1, 'tool_use', { toolUse: { id: 'call', name: 'bash', input: { command: 'printf ok' } } }));
  expect(() => guard.accept(frame(2, 'tool_permission_requested', { call_id: 'call', permission_request_id: 'p', tool_name: 'foreign' }))).toThrow('permission_correlation');
  expect(guard.accept(frame(2, 'tool_permission_requested', { call_id: 'call', permission_request_id: 'p', tool_name: 'bash' })).permission?.input).toEqual({ command: 'printf ok' });
});

test('one permission identity cannot target two tool calls', () => {
  const guard = pendingTool();
  guard.accept(frame(3, 'tool_use', { toolUse: { id: 'other-call', name: 'bash', input: {} } }));
  expect(() => guard.accept(frame(4, 'tool_permission_requested', { call_id: 'other-call', permission_request_id: 'p' }))).toThrow('permission_correlation');
});

test('an unpermissioned call cannot consume another calls permission via a result', () => {
  const guard = pendingTool();
  guard.accept(frame(3, 'tool_use', { toolUse: { id: 'other-call', name: 'bash', input: {} } }));
  expect(() => guard.accept(frame(4, 'tool_result', { call_id: 'other-call', permission_request_id: 'p', outcome: 'success' }))).toThrow('permission_correlation');
});

test('tool outcome must be a string enum, not a coercible JSON value', () => {
  expect(() => pendingTool().accept(frame(3, 'tool_result', { call_id: 'call', permission_request_id: 'p', outcome: ['success'] }))).toThrow('tool_correlation');
});

test('tool output cap includes the truncation marker and line count', () => {
  const event = pendingTool().accept(frame(3, 'tool_result', { call_id: 'call', permission_request_id: 'p', outcome: 'success', output: 'line\n'.repeat(1000) }));
  expect(event.tool?.output!.split('\n').length).toBeLessThanOrEqual(20);
});
