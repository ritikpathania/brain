import { expect, test } from 'bun:test';
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';

type WorkflowStep = { run?: string; if?: unknown; 'continue-on-error'?: unknown; uses?: string; with?: { repository?: unknown; ref?: unknown; 'bun-version'?: string } };
type Workflow = { jobs: { 'canonical-frontend': { if?: unknown; steps: WorkflowStep[] } } };

test('canonical CI runs the owned package without external-source conditions or failure suppression', async () => {
  const workflow = Bun.YAML.parse(await Bun.file(resolve(import.meta.dir, '../../../../.github/workflows/ci.yml')).text()) as Workflow;
  const job = workflow.jobs['canonical-frontend'];
  expect(job.if).toBeUndefined();
  for (const step of job.steps) {
    expect(step.if).toBeUndefined();
    expect(step['continue-on-error']).toBeUndefined();
    if (step.uses?.startsWith('actions/checkout')) {
      expect(step.with?.repository).toBeUndefined();
      expect(step.with?.ref).toBeUndefined();
    }
  }
  const bunStep = job.steps.find(step => step.uses?.startsWith('oven-sh/setup-bun'));
  expect(bunStep).toBeDefined();
  expect(bunStep?.with?.['bun-version']).toBe('1.4.4');
  expect(job.steps.some(step => step.run?.includes('./scripts/verify-frontend-e2e.sh'))).toBe(true);
});

test('the shared frontend E2E gate requires both binaries, passing tests, and zero skips', async () => {
  const root = mkdtempSync(join(tmpdir(), 'bfc-'));
  const gate = resolve(import.meta.dir, '../../../../scripts/verify-frontend-e2e.sh');
  const bashEnv = join(root, 'bash-env');
  try {
    writeFileSync(bashEnv, "bun() { printf '%s\\n' \"$FIXTURE_OUTPUT\" >&2; return \"$FIXTURE_EXIT\"; }\n");
    for (const [output, runnerExit, expectedExit] of [
      ['11 pass\n0 fail\n0 skip', 0, 0],
      ['11 pass\n1 skip', 0, 1],
      ['(skip) excluded\n11 pass', 0, 1],
      ['11 pass\n1 todo', 0, 1],
      ['(todo) excluded\n11 pass', 0, 1],
      ['0 pass\n0 fail', 0, 1],
      ['1 fail', 23, 23],
    ] as const) {
      // Execute the shared shell gate. Only the expensive test runner is
      // controlled; assert the consumer-visible exit code, not fixture calls.
      const child = Bun.spawn(['bash', gate], {
        env: { ...process.env, BASH_ENV: bashEnv, HOME: root, XDG_CONFIG_HOME: root,
          BRAIN_CLI_BIN: process.execPath, BRAIN_DAEMON_BIN: process.execPath,
          FIXTURE_OUTPUT: output, FIXTURE_EXIT: String(runnerExit) },
        stdout: 'pipe', stderr: 'pipe',
      });
      const [stdout, stderr, exit] = await Promise.all([
        new Response(child.stdout).text(), new Response(child.stderr).text(), child.exited,
      ]);
      expect({ output, exit, diagnostics: exit === expectedExit ? '' : stdout + stderr })
        .toEqual({ output, exit: expectedExit, diagnostics: '' });
    }
    const missingDaemon = Bun.spawnSync(['bash', gate], {
      env: { ...process.env, BASH_ENV: bashEnv, BRAIN_CLI_BIN: process.execPath,
        BRAIN_DAEMON_BIN: join(root, 'missing'), FIXTURE_OUTPUT: '11 pass', FIXTURE_EXIT: '0' },
    });
    expect(missingDaemon.exitCode).toBe(2);
    expect(missingDaemon.stderr.toString()).toContain('BRAIN_DAEMON_BIN');
  } finally { rmSync(root, { recursive: true, force: true }); }
});
