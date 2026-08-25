import { describe, it, expect, beforeEach } from 'bun:test';
import * as fs from 'fs';
import * as os from 'os';
import * as path from 'path';
import { SessionController } from '../../state/sessionController.js';
import type {
  BrainBackendClient,
  BrainStreamChunk,
  BrainGenerationRequest,
} from '../../client/BrainBackendClient.js';

const sleep = (ms: number): Promise<void> => new Promise((r) => setTimeout(r, ms));

let cfgPath: string;

beforeEach(() => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'brain-inc17-ctl-'));
  cfgPath = path.join(dir, 'config.json');
  process.env.BRAIN_CONFIG_PATH = cfgPath;
});

function seedRule(): void {
  fs.writeFileSync(
    cfgPath,
    JSON.stringify({
      theme: 'dark',
      permissions: { allow: [{ tool: 'bash', inputPrefix: 'git ' }] },
    }),
  );
}

interface Resolution {
  callId: string;
  granted: boolean;
}

function recordingClient(
  chunks: BrainStreamChunk[],
  opts: { rejectResolve?: boolean } = {},
): { client: BrainBackendClient; resolutions: Resolution[] } {
  const resolutions: Resolution[] = [];
  const client = {
    async createSession() {
      return { sessionId: 'perm-probe', title: 't', createdAtMs: 0 };
    },
    async *streamText(_req: BrainGenerationRequest): AsyncIterable<BrainStreamChunk> {
      yield* chunks;
    },
    async resolveToolPermission(callId: string, granted: boolean): Promise<void> {
      if (opts.rejectResolve) {
        throw new Error('Brain daemon socket error on v1/tool/resolve: boom');
      }
      resolutions.push({ callId, granted });
    },
  } as unknown as BrainBackendClient;
  return { client, resolutions };
}

function permChunk(command: string, callId = 'c1'): BrainStreamChunk {
  return {
    type: 'permission_request',
    callId,
    toolName: 'bash',
    input: { command },
  } as unknown as BrainStreamChunk;
}

function systemText(ctl: SessionController): string {
  return ctl
    .getSnapshot()
    .rows.filter((r) => r.kind === 'system')
    .map((r) => r.text)
    .join('\n');
}

describe('Inc 17: controller auto-allow from saved rules', () => {
  it('auto-allows a matching rule without parking the dialog', async () => {
    seedRule();
    const { client, resolutions } = recordingClient([
      permChunk('git status'),
      { type: 'token', token: 'clean tree' },
      { type: 'finished', status: 'completed' },
    ]);
    const ctl = new SessionController(client);
    await ctl.submit('check the repo');
    expect(ctl.getSnapshot().permission).toBeUndefined();
    expect(resolutions).toEqual([{ callId: 'c1', granted: true }]);
    expect(systemText(ctl)).toContain('Allowed bash (rule 1)');
    ctl.dispose();
  });

  it('parks unmatched requests and resolves them manually as before', async () => {
    const { client, resolutions } = recordingClient([
      permChunk('rm -rf build'),
      { type: 'finished', status: 'completed' },
    ]);
    const ctl = new SessionController(client);
    await ctl.submit('clean up');
    expect(ctl.getSnapshot().permission?.callId).toBe('c1');
    expect(resolutions).toEqual([]);
    ctl.resolvePermission('c1', true);
    expect(resolutions).toEqual([{ callId: 'c1', granted: true }]);
    expect(ctl.getSnapshot().permission).toBeUndefined();
    ctl.dispose();
  });

  it('re-parks the dialog when the wire verdict fails to deliver', async () => {
    seedRule();
    const { client } = recordingClient(
      [
        permChunk('git push'),
        { type: 'token', token: 'partial' },
        { type: 'finished', status: 'completed' },
      ],
      { rejectResolve: true },
    );
    const ctl = new SessionController(client);
    await ctl.submit('push it');
    // The rejected verdict may park the dialog anywhere from mid-stream to
    // just after submit settles; only the settled outcome is contractual.
    await sleep(5); // let the rejected promise route through the fallback
    expect(ctl.getSnapshot().permission?.callId).toBe('c1');
    ctl.dispose();
  });

  it('resolvePermissionAlways persists the derived rule and grants', async () => {
    const { client, resolutions } = recordingClient([
      permChunk('git fetch', 'c9'),
      { type: 'finished', status: 'completed' },
    ]);
    const ctl = new SessionController(client);
    await ctl.submit('fetch refs');
    ctl.resolvePermissionAlways('c9');
    expect(resolutions).toEqual([{ callId: 'c9', granted: true }]);
    const saved = JSON.parse(fs.readFileSync(cfgPath, 'utf8')) as {
      permissions?: { allow?: Array<{ tool: string; inputPrefix: string }> };
    };
    // The rule stores the full derived primary string, per design §3.
    expect(saved.permissions?.allow).toEqual([{ tool: 'bash', inputPrefix: 'git fetch' }]);

    // The saved rule takes effect on the very next request.
    await ctl.submit('fetch again');
    expect(ctl.getSnapshot().permission).toBeUndefined();
    expect(resolutions).toEqual([
      { callId: 'c9', granted: true },
      { callId: 'c9', granted: true },
    ]);
    expect(systemText(ctl)).toContain('Allowed bash (rule 1)');
    ctl.dispose();
  });

  it('still grants this call when saving the rule fails', async () => {
    process.env.BRAIN_CONFIG_PATH = path.join(os.tmpdir(), 'brain-inc17-dir-not-file');
    fs.rmSync(process.env.BRAIN_CONFIG_PATH!, { recursive: true, force: true });
    fs.mkdirSync(process.env.BRAIN_CONFIG_PATH!); // configPath() now names a directory
    const { client, resolutions } = recordingClient([
      permChunk('git log', 'cf'),
      { type: 'finished', status: 'completed' },
    ]);
    const ctl = new SessionController(client);
    await ctl.submit('show log');
    ctl.resolvePermissionAlways('cf');
    expect(systemText(ctl)).toContain('Could not save the always-allow rule.');
    expect(resolutions).toEqual([{ callId: 'cf', granted: true }]);
    expect(ctl.getSnapshot().permission).toBeUndefined();
    ctl.dispose();
  });

  it('derives a tool-wide rule from inputs without a string field', async () => {
    const { client, resolutions } = recordingClient([
      permChunk('', 'ce'),
      { type: 'finished', status: 'completed' },
    ]);
    const ctl = new SessionController(client);
    await ctl.submit('go');
    ctl.resolvePermissionAlways('ce'); // input {command:''} has no primary string
    const saved = JSON.parse(fs.readFileSync(cfgPath, 'utf8')) as {
      permissions?: { allow?: Array<{ tool: string; inputPrefix: string }> };
    };
    expect(saved.permissions?.allow).toEqual([{ tool: 'bash', inputPrefix: '' }]);
    ctl.dispose();
  });
});
