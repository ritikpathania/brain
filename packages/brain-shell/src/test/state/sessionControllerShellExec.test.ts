import { describe, it, expect } from 'bun:test';
import { SessionController } from '../../state/sessionController.js';
import type {
  BrainBackendClient,
  CreateSessionResponse,
  ShellExecResult,
} from '../../client/BrainBackendClient.js';

function fakeExecClient(
  execImpl: (sessionId: string, command: string, signal?: AbortSignal) => Promise<ShellExecResult>,
) {
  const client = {
    async createSession(): Promise<CreateSessionResponse> {
      return { sessionId: 'stub-session-x', title: 'stub', createdAtMs: 0 };
    },
    execShell,
  } as unknown as BrainBackendClient;
  function execShell(
    sessionId: string,
    command: string,
    signal?: AbortSignal,
  ): Promise<ShellExecResult> {
    return execImpl(sessionId, command, signal);
  }
  return client;
}

function ok(command: string, exitCode = 0, output = ''): ShellExecResult {
  return {
    callId: `shell-${command.length}`,
    name: 'bash',
    input: { command },
    outcome: 'executed',
    output: output || `${command}\n`,
    isError: exitCode !== 0,
    exitCode,
    durationMs: 42,
  };
}

describe('runShellCommand (Inc 11)', () => {
  it('pushes the user line, projects a completed card, restores idle', async () => {
    const ctl = new SessionController(fakeExecClient((_sid, cmd) => ok(cmd)));
    await ctl.runShellCommand('echo bang');

    const snap = ctl.getSnapshot();
    expect(snap.rows[0]).toMatchObject({ kind: 'user', text: '! echo bang' });
    const cardRow = snap.rows.find((r) => r.kind === 'tool');
    expect(cardRow).toBeDefined();
    if (cardRow?.kind === 'tool') {
      expect(cardRow.tool.toolName).toBe('bash');
      expect(cardRow.tool.status).toBe('completed');
      expect(cardRow.tool.output).toBe('echo bang\n');
      expect(cardRow.tool.durationMs).toBe(42);
      expect(cardRow.tool.exitCode).toBe(0);
    }
    expect(snap.busy).toBe(false);
    expect(snap.live.phase).toBe('idle');
  });

  it('projects a failed card carrying the real exit code', async () => {
    const ctl = new SessionController(fakeExecClient((_sid, cmd) => ok(cmd, 2, 'boom')));
    await ctl.runShellCommand('false');

    const snap = ctl.getSnapshot();
    const cardRow = snap.rows.find((r) => r.kind === 'tool');
    if (cardRow?.kind === 'tool') {
      expect(cardRow.tool.status).toBe('failed');
      expect(cardRow.tool.isError).toBe(true);
      expect(cardRow.tool.exitCode).toBe(2);
      expect(cardRow.tool.output).toBe('boom');
    } else {
      throw new Error('expected a tool row');
    }
  });

  it('rejects a second command while busy with a visible notice', async () => {
    let release!: (r: ShellExecResult) => void;
    const gate = new Promise<ShellExecResult>((res) => (release = res));
    const ctl = new SessionController(fakeExecClient(() => gate));

    const first = ctl.runShellCommand('sleepish');
    await new Promise((r) => setTimeout(r, 20)); // let busy flip
    expect(ctl.getSnapshot().busy).toBe(true);

    await ctl.runShellCommand('echo again'); // dropped with a notice
    const notices = ctl.getSnapshot().rows.filter((r) => r.kind === 'system');
    expect(notices.some((n) => n.kind === 'system' && n.text.includes('Busy'))).toBe(true);

    release(ok('sleepish'));
    await first;
    expect(ctl.getSnapshot().busy).toBe(false);
  });

  it('surfaces backend rejections as a notice and stays usable', async () => {
    const ctl = new SessionController(
      fakeExecClient(() => Promise.reject(new Error('Brain daemon RPC timeout (35000ms) on v1/shell/exec'))),
    );
    await ctl.runShellCommand('whatever');
    const snap = ctl.getSnapshot();
    expect(snap.rows.some((r) => r.kind === 'system' && r.text.includes('Could not run command'))).toBe(true);
    expect(snap.busy).toBe(false);
  });

  it('esc-abort during exec lands the cancelled notice', async () => {
    const ctl = new SessionController(
      fakeExecClient(
        (_sid, _cmd, signal) =>
          new Promise((_res, rej) => {
            signal?.addEventListener('abort', () => rej(new Error('v1/shell/exec aborted')), { once: true });
          }),
      ),
    );
    const pending = ctl.runShellCommand('long-runner');
    await new Promise((r) => setTimeout(r, 20));
    ctl.abort();
    await pending;
    const snap = ctl.getSnapshot();
    expect(snap.rows.some((r) => r.kind === 'system' && r.text.includes('cancelled'))).toBe(true);
    expect(snap.busy).toBe(false);
  });
});
