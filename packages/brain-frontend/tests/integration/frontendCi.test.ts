import { expect, test } from 'bun:test';
import { mkdirSync, mkdtempSync, readFileSync, readdirSync, realpathSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';

type WorkflowStep = {
  name?: string;
  id?: string;
  env?: Record<string, string>;
  run?: string;
  if?: unknown;
  'continue-on-error'?: unknown;
  uses?: string;
  with?: { repository?: unknown; ref?: unknown; 'bun-version'?: string; version?: string };
};
type Workflow = {
  jobs: {
    'canonical-frontend': { if?: unknown; steps: WorkflowStep[] };
    'sdk-and-packaging': { if?: unknown; steps: WorkflowStep[] };
    'xtask-verify-gate': { if?: unknown; steps: WorkflowStep[] };
  };
};

test('the product checkout excludes the retired shell and its manual PTY entry points', () => {
  const repository = resolve(import.meta.dir, '../../../..');
  const tracked = Bun.spawnSync([
    'git', '-c', 'core.fsmonitor=false', 'ls-files', '--',
    'packages/brain-shell', ':(glob)scripts/ptySmokeInc*.py',
  ], { cwd: repository });
  expect(tracked.exitCode).toBe(0);
  expect(tracked.stdout.toString()).toBe('');
});

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

test('ordinary CI sdk-and-packaging provisions the locked frontend Bun version', async () => {
  const workflow = Bun.YAML.parse(await Bun.file(resolve(import.meta.dir, '../../../../.github/workflows/ci.yml')).text()) as Workflow;
  const job = workflow.jobs['sdk-and-packaging'];
  expect(job).toBeDefined();

  const bunStep = job.steps.find(step => step.uses?.startsWith('oven-sh/setup-bun'));
  expect(bunStep).toBeDefined();
  expect(bunStep?.with?.['bun-version']).toBe('1.4.4');
});

test('ordinary SDK and packaging consumers use the release-pinned standalone interpreter', async () => {
  const workflow = Bun.YAML.parse(await Bun.file(resolve(import.meta.dir, '../../../../.github/workflows/ci.yml')).text()) as Workflow;
  const { steps } = workflow.jobs['sdk-and-packaging'];
  const uv = steps.findIndex(step => step.uses?.startsWith('astral-sh/setup-uv'));
  const python = steps.findIndex(step => step.id === 'python');
  expect(uv).toBeGreaterThanOrEqual(0);
  expect(steps[uv]?.with?.version).toBe('0.13.8');
  expect(python).toBeGreaterThan(uv);
  expect(steps.some(step => step.uses?.startsWith('actions/setup-python'))).toBe(false);
  expect(steps[python]?.run).toContain('uv sync --frozen --no-install-project --managed-python --python "$(cat .python-version)"');
  expect(steps[python]?.run).toContain('.venv/bin/python');
  for (const command of ['pytest', 'bash package.sh']) {
    const consumer = steps.findIndex(step => step.run?.includes(command));
    expect(consumer).toBeGreaterThan(python);
    expect(steps[consumer]?.env?.PYO3_PYTHON).toBe('${{ steps.python.outputs.python-path }}');
  }
  expect(steps.find(step => step.run?.includes('pytest'))?.run).toContain('uv run --no-sync pytest');
});

test('the ordinary quality gate keeps Rust PyO3 verification without an undeclared Maturin build', async () => {
  const workflow = Bun.YAML.parse(await Bun.file(resolve(import.meta.dir, '../../../../.github/workflows/ci.yml')).text()) as Workflow;
  for (const job of ['xtask-verify-gate', 'sdk-and-packaging'] as const) {
    expect(workflow.jobs[job].steps.some(step => step.run?.includes('maturin'))).toBe(false);
  }
  const gate = workflow.jobs['xtask-verify-gate'].steps.find(step => step.run?.includes('cargo xtask verify'));
  expect(gate).toBeDefined();
  expect(gate?.['continue-on-error']).toBeUndefined();
  expect(gate?.if).toBeUndefined();
});

test('ordinary CI verifies a large product archive and propagates missing members and command failures', async () => {
  const workflow = Bun.YAML.parse(await Bun.file(resolve(import.meta.dir, '../../../../.github/workflows/ci.yml')).text()) as Workflow;
  const script = workflow.jobs['sdk-and-packaging'].steps.find(step => step.run?.includes('bash package.sh'))?.run;
  expect(script).toBeDefined();
  const root = realpathSync(mkdtempSync(join(tmpdir(), 'bfc package ')));
  const required = [
    'brain-dist/bin/brain',
    'brain-dist/bin/brain-daemon',
    'brain-dist/frontend/brain-frontend/brain-frontend',
    'brain-dist/product-manifest.json',
  ];
  try {
    const source = join(root, 'source');
    for (const path of ['brain-dist/bin', 'brain-dist/frontend/brain-frontend', 'brain-dist/runtime']) {
      mkdirSync(join(source, path), { recursive: true });
    }
    writeFileSync(join(source, required[0]!), '#!/bin/bash\nprintf "health:%s:%s\\n" "$0" "$*" >> "$CI_TRACE"\nexit "$CI_HEALTH_EXIT"\n', { mode: 0o755 });
    writeFileSync(join(source, required[1]!), '#!/bin/bash\nprintf "daemon:%s:%s\\n" "$0" "$*" >> "$CI_TRACE"\nexit "$CI_DAEMON_EXIT"\n', { mode: 0o755 });
    writeFileSync(join(source, required[2]!), 'standalone frontend fixture\n');
    writeFileSync(join(source, required[3]!), '{}\n');
    // A runtime-sized listing makes an early grep -q match close the pipe
    // while the manifest writer still has data, exposing SIGPIPE with pipefail.
    for (let i = 0; i < 2048; i++) {
      writeFileSync(join(source, 'brain-dist/runtime', `${i}-${'metadata'.repeat(16)}`), '');
    }
    for (const scenario of [
      { name: 'valid archive', missing: '', packageExit: 0, daemonExit: 0, healthExit: 1, want: 0, calls: 2 },
      ...required.map(missing => ({ name: `missing ${missing}`, missing, packageExit: 0, daemonExit: 0, healthExit: 1, want: 1, calls: 0 })),
      { name: 'package failure', missing: '', packageExit: 83, daemonExit: 0, healthExit: 1, want: 83, calls: 0 },
      { name: 'daemon failure', missing: '', packageExit: 0, daemonExit: 84, healthExit: 1, want: 84, calls: 1 },
      { name: 'health failure', missing: '', packageExit: 0, daemonExit: 0, healthExit: 85, want: 1, calls: 2 },
    ]) {
      const runRoot = mkdtempSync(join(root, 'run '));
      const runnerTemp = join(runRoot, 'runner temp');
      const archive = join(runRoot, 'fixture.tar.gz');
      const trace = join(runRoot, 'trace');
      mkdirSync(runnerTemp);
      writeFileSync(trace, '');
      const tar = Bun.spawnSync(['tar', '-czf', archive, '-C', source,
        ...required.filter(path => path !== scenario.missing),
        ...(scenario.name === 'valid archive' ? ['brain-dist/runtime'] : [])]);
      expect({ exit: tar.exitCode, stderr: tar.stderr.toString() }).toEqual({ exit: 0, stderr: '' });
      writeFileSync(join(runRoot, 'package.sh'), '#!/bin/bash\nset -eu\nif [[ "$CI_PACKAGE_EXIT" != 0 ]]; then exit "$CI_PACKAGE_EXIT"; fi\ncp "$CI_ARCHIVE" brain-fixture.tar.gz\n');
      const run = Bun.spawnSync(['bash', '--noprofile', '--norc', '-e', '-o', 'pipefail', '-c', script!], {
        cwd: runRoot,
        env: { ...process.env, BASH_ENV: '', RUNNER_TEMP: runnerTemp, HOME: runRoot,
          XDG_CONFIG_HOME: join(runRoot, 'config'), CI_ARCHIVE: archive, CI_TRACE: trace,
          CI_PACKAGE_EXIT: String(scenario.packageExit), CI_DAEMON_EXIT: String(scenario.daemonExit),
          CI_HEALTH_EXIT: String(scenario.healthExit) },
      });
      expect({ scenario: scenario.name, exit: run.exitCode, stderr: run.stderr.toString() })
        .toEqual({ scenario: scenario.name, exit: scenario.want, stderr: '' });
      const calls = readFileSync(trace, 'utf8').trim().split('\n').filter(Boolean);
      expect(calls).toHaveLength(scenario.calls);
      if (calls.length > 0) {
        expect(calls[0]).toStartWith(`daemon:${runnerTemp}/`);
        expect(calls[0]).toEndWith('/brain-dist/bin/brain-daemon:--version');
      }
      if (calls.length > 1) {
        expect(calls[1]).toStartWith(`health:${runnerTemp}/`);
        expect(calls[1]).toEndWith('/brain-dist/bin/brain:health');
      }
      expect(readdirSync(runnerTemp)).toEqual([]);
    }
  } finally { rmSync(root, { recursive: true, force: true }); }
}, 30_000);

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
