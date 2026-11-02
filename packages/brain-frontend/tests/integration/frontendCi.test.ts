import { expect, test } from 'bun:test';
import { mkdtempSync, rmSync } from 'node:fs';
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
  const gate = job.steps.find(step => step.run?.includes('bun run test:e2e'))?.run;
  expect(gate).toBeDefined();
  const root = mkdtempSync(join(tmpdir(), 'bfc-'));
  try {
    for (const [output, runnerExit, expectedExit] of [
      ['11 pass\n0 fail\n0 skip', 0, 0],
      ['1 skip', 0, 1],
      ['(skip) excluded', 0, 1],
      ['1 todo', 0, 1],
      ['1 fail', 23, 23],
    ] as const) {
      // Execute the workflow's real shell gate. Only the expensive test runner is
      // controlled; assert the consumer-visible exit code, not fixture calls.
      const command = `bun() { printf '%s\\n' "$FIXTURE_OUTPUT" >&2; return "$FIXTURE_EXIT"; };\n${gate}`;
      const child = Bun.spawn(['bash', '-c', command], {
        env: { ...process.env, HOME: root, XDG_CONFIG_HOME: root,
          FIXTURE_OUTPUT: output, FIXTURE_EXIT: String(runnerExit) },
        stdout: 'pipe', stderr: 'pipe',
      });
      const [stdout, stderr, exit] = await Promise.all([
        new Response(child.stdout).text(), new Response(child.stderr).text(), child.exited,
      ]);
      expect({ output, exit, diagnostics: exit === expectedExit ? '' : stdout + stderr })
        .toEqual({ output, exit: expectedExit, diagnostics: '' });
    }
  } finally { rmSync(root, { recursive: true, force: true }); }
});
