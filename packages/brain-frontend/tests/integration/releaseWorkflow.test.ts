import { expect, test } from 'bun:test';
import { copyFileSync, mkdtempSync, mkdirSync, readFileSync, realpathSync, rmSync, symlinkSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';

const repositoryRoot = resolve(import.meta.dir, '../../../..');

type WorkflowStep = {
  id?: string;
  name?: string;
  run?: string;
  if?: unknown;
  'continue-on-error'?: unknown;
  uses?: string;
  with?: Record<string, unknown>;
  shell?: string;
  env?: Record<string, string>;
};

type WorkflowJob = {
  name?: string;
  if?: string;
  needs?: string | string[];
  runs_on?: string;
  strategy?: {
    'fail-fast'?: boolean;
    matrix?: {
      include?: Array<{
        os: string;
        target: string;
        artifact_name: string;
      }>;
    };
  };
  steps: WorkflowStep[];
};

type ReleaseWorkflow = {
  on: { push: { branches: string[]; tags: string[] }; workflow_dispatch?: unknown };
  jobs: {
    'release-quality': WorkflowJob;
    'build-binaries': WorkflowJob;
    'publish-release': WorkflowJob;
  };
};

async function loadWorkflow(): Promise<ReleaseWorkflow> {
  return Bun.YAML.parse(await Bun.file(join(repositoryRoot, '.github/workflows/release.yml')).text()) as ReleaseWorkflow;
}

test('release workflow defines unified product matrix, blocking quality, and product verification without external authorization guards', async () => {
  const workflow = await loadWorkflow();

  // 1. release-quality job assertions
  const quality = workflow.jobs['release-quality'];
  expect(quality).toBeDefined();
  const qualitySteps = quality.steps;
  const docsStep = qualitySteps.find((s) => s.run?.includes('cargo xtask docs all'));
  expect(docsStep).toBeDefined();
  expect(docsStep?.['continue-on-error']).toBeUndefined();

  // 2. build-binaries matrix assertions
  const buildJob = workflow.jobs['build-binaries'];
  expect(buildJob).toBeDefined();
  const matrix = buildJob.strategy?.matrix?.include;
  expect(matrix).toBeDefined();
  expect(matrix).toHaveLength(2);
  const targets = matrix!.map((m) => m.target).sort();
  expect(targets).toEqual(['aarch64-apple-darwin', 'x86_64-unknown-linux-gnu']);

  // Ensure the native jobs remain independent and Bun is pinned.
  expect(buildJob.strategy?.['fail-fast']).toBe(false);
  const bunStep = buildJob.steps.find((s) => s.uses?.startsWith('oven-sh/setup-bun'));
  expect(bunStep).toBeDefined();
  expect(bunStep?.with?.['bun-version']).toBe('1.4.4');

  // Verify product packaging and clean extraction smoke
  const packageStep = buildJob.steps.find((s) => s.run?.includes('package-brain-product.sh'));
  expect(packageStep).toBeDefined();

  const smokeStep = buildJob.steps.find((s) => s.run?.includes('verify-brain-product.sh'));
  expect(smokeStep).toBeDefined();
  expect(smokeStep?.run).toContain('--archive');

  // Assert no continue-on-error or external release authorization flags
  for (const step of buildJob.steps) {
    expect(step['continue-on-error']).toBeUndefined();
    if (step.run) {
      expect(step.run).not.toContain('BRAIN_AUTHORIZED_FRONTEND_RELEASE');
    }
    if (step.uses?.startsWith('actions/checkout')) {
      expect(step.with?.repository).toBeUndefined();
    }
  }

  // 3. publish-release dependency assertions
  const publish = workflow.jobs['publish-release'];
  expect(publish).toBeDefined();
  const needs = Array.isArray(publish.needs) ? publish.needs : [publish.needs];
  expect(needs.sort()).toEqual(['build-binaries', 'release-quality']);
});

test('release quality selects pinned CPython before each all-features Rust gate', async () => {
  const steps = (await loadWorkflow()).jobs['release-quality'].steps;
  const setupPython = steps.findIndex(step => step.uses?.startsWith('actions/setup-python'));
  expect(setupPython).toBeGreaterThanOrEqual(0);
  expect(steps[setupPython]?.with?.['python-version']).toBe('3.15.2');
  const gates = steps.filter(step => step.run?.includes('--all-features'));
  expect(gates).toHaveLength(2);
  for (const gate of gates) {
    expect(steps.indexOf(gate)).toBeGreaterThan(setupPython);
    expect(gate.env?.PYO3_PYTHON).toBe('${{ steps.python.outputs.python-path }}');
  }
});

test('native builds and package verification share the project-pinned standalone CPython', async () => {
  const { steps } = (await loadWorkflow()).jobs['build-binaries'];
  const uv = steps.findIndex(step => step.uses?.startsWith('astral-sh/setup-uv'));
  const python = steps.findIndex(step => step.id === 'python');
  expect(uv).toBeGreaterThanOrEqual(0);
  expect(steps[uv]?.with?.version).toBe('0.13.8');
  expect(python).toBeGreaterThan(uv);
  expect(steps.some(step => step.uses?.startsWith('actions/setup-python'))).toBe(false);
  expect(steps[python]?.run).toContain('$(cat .python-version)');
  expect(steps[python]?.run).toContain('uv sync --frozen --no-install-project --managed-python');
  expect(steps[python]?.run).toContain('.venv/bin/python');

  for (const command of ['cargo build --release', 'package-brain-product.sh', 'verify-brain-product.sh']) {
    const consumer = steps.findIndex(step => step.run?.includes(command));
    expect(consumer).toBeGreaterThan(python);
    expect(steps[consumer]?.env?.PYO3_PYTHON).toBe('${{ steps.python.outputs.python-path }}');
  }
  const e2e = steps.find(step => step.run?.includes('verify-frontend-e2e.sh'));
  const libraryDir = '${{ steps.python.outputs.python-lib-dir }}';
  expect(e2e?.env?.LD_LIBRARY_PATH).toBe(libraryDir);
  expect(e2e?.env?.DYLD_FALLBACK_LIBRARY_PATH).toBe(libraryDir);
});

test('each release matrix target requires native frontend E2E before standalone compilation', async () => {
  const steps = (await loadWorkflow()).jobs['build-binaries'].steps;
  const native = steps.findIndex(step => step.run?.includes('cargo build --release --locked'));
  const e2e = steps.findIndex(step => step.run?.includes('./scripts/verify-frontend-e2e.sh'));
  const standalone = steps.findIndex(step => step.run?.includes('bun run build'));
  expect(native).toBeGreaterThanOrEqual(0);
  expect(e2e).toBeGreaterThan(native);
  expect(standalone).toBeGreaterThan(e2e);
  expect(steps[e2e]?.if).toBeUndefined();
  expect(steps[e2e]?.env?.BRAIN_CLI_BIN).toBe('${{ github.workspace }}/target/${{ matrix.target }}/release/brain');
  expect(steps[e2e]?.env?.BRAIN_DAEMON_BIN).toBe('${{ github.workspace }}/target/${{ matrix.target }}/release/brain-daemon');
});

test('release verification can be dispatched or run from an opt-in branch while only tag pushes publish', async () => {
  const workflow = await loadWorkflow();
  expect(workflow.on).toHaveProperty('workflow_dispatch');
  expect(workflow.on.push.branches).toEqual(['release-verify/**']);
  expect(workflow.on.push.tags).toEqual(['v*']);
  expect(workflow.jobs['publish-release'].if).toBe("github.event_name == 'push' && startsWith(github.ref, 'refs/tags/v')");
});

test('local packaging stops at a failed frontend E2E before standalone compilation', () => {
  const root = realpathSync(mkdtempSync(join(tmpdir(), 'bfp release ')));
  const bashEnv = join(root, 'bash-env');
  const trace = join(root, 'trace');
  const flags = join(root, 'flags');
  try {
    for (const path of ['daemon/.venv/bin', 'packages/brain-frontend', 'scripts']) {
      mkdirSync(join(root, path), { recursive: true });
    }
    copyFileSync(join(repositoryRoot, 'package.sh'), join(root, 'package.sh'));
    writeFileSync(join(root, 'daemon/.venv/bin/python'), '#!/bin/bash\nprintf "/fixture/lib\\n"\n', { mode: 0o755 });
    writeFileSync(bashEnv, `
rustc() { printf 'host: aarch64-apple-darwin\\n'; }
cargo() {
  printf 'cargo %s\\n' "$*" >> "$RELEASE_TRACE"
  if [[ "$1" == build ]]; then printf '%s' "$CARGO_ENCODED_RUSTFLAGS" > "$RELEASE_FLAGS"; fi
}
bun() { printf 'bun %s\\n' "$*" >> "$RELEASE_TRACE"; }
`);
    writeFileSync(join(root, 'scripts/verify-frontend-e2e.sh'), `#!/bin/bash
set -eu
[[ "$BRAIN_CLI_BIN" == "$RELEASE_ROOT/target/release/brain" ]]
[[ "$BRAIN_DAEMON_BIN" == "$RELEASE_ROOT/target/release/brain-daemon" ]]
printf 'frontend e2e\\n' >> "$RELEASE_TRACE"
exit 83
`, { mode: 0o755 });
    const run = Bun.spawnSync(['bash', join(root, 'package.sh')], {
      env: { ...process.env, BASH_ENV: bashEnv, RELEASE_ROOT: root, RELEASE_TRACE: trace, RELEASE_FLAGS: flags,
        CARGO_ENCODED_RUSTFLAGS: '--cfg\x1fbrain_test', RUSTFLAGS: '' },
    });
    expect({ exit: run.exitCode, trace: readFileSync(trace, 'utf8').trim().split('\n') }).toEqual({
      exit: 83,
      trace: ['cargo xtask verify', 'cargo xtask docs all', 'bun install --frozen-lockfile', 'bun run typecheck',
        'bun run test', 'cargo build --release --package brain --package brain-daemon --locked', 'frontend e2e'],
    });
    const encodedFlags = readFileSync(flags, 'utf8').split('\x1f');
    expect(encodedFlags.slice(0, 2)).toEqual(['--cfg', 'brain_test']);
    expect(encodedFlags).toContain(`--remap-path-prefix=${root}=.`);
  } finally { rmSync(root, { recursive: true, force: true }); }
});

test('local packaging supplies standalone Python loader paths only to source-binary tests', () => {
  const root = realpathSync(mkdtempSync(join(tmpdir(), 'bfp loader paths ')));
  try {
    for (const prior of [
      { ld: '', dyld: '' },
      { ld: '/prior/linux/first:/prior/linux/second', dyld: '/prior/darwin/first:/prior/darwin/second' },
    ]) {
      const runRoot = mkdtempSync(join(root, 'run '));
      const trace = join(runRoot, 'trace');
      const libraryDir = join(runRoot, 'standalone python/lib');
      const bashEnv = join(runRoot, 'bash-env');
      for (const path of ['daemon/.venv/bin', 'packages/brain-frontend', 'scripts', 'standalone python/lib']) {
        mkdirSync(join(runRoot, path), { recursive: true });
      }
      copyFileSync(join(repositoryRoot, 'package.sh'), join(runRoot, 'package.sh'));
      writeFileSync(join(runRoot, 'daemon/.venv/bin/python'), '#!/bin/bash\nprintf "%s\\n" "$RELEASE_LIBRARY"\n', { mode: 0o755 });
      writeFileSync(bashEnv, `
rustc() { printf 'host: x86_64-unknown-linux-gnu\\n'; }
cargo() {
  if [[ "$*" == 'xtask verify' ]]; then
    printf '%s\\n' "\${LD_LIBRARY_PATH-}" "\${DYLD_FALLBACK_LIBRARY_PATH-}" > "$RELEASE_GATE_PATHS"
  fi
}
bun() { :; }
mktemp() { command mktemp -d "$RELEASE_ROOT/staging.XXXXXX"; }
if [[ -n "$RELEASE_PRIOR_LD" ]]; then
  export LD_LIBRARY_PATH="$RELEASE_PRIOR_LD"
else
  unset LD_LIBRARY_PATH
fi
if [[ -n "$RELEASE_PRIOR_DYLD" ]]; then
  export DYLD_FALLBACK_LIBRARY_PATH="$RELEASE_PRIOR_DYLD"
else
  unset DYLD_FALLBACK_LIBRARY_PATH
fi
`);
      // Bun fixtures observe the actual child environment without macOS's
      // protected system-shell loader-variable filtering hiding the boundary.
      const stageFixture = `#!${process.execPath}
import { appendFileSync, existsSync, mkdirSync, writeFileSync } from 'node:fs';
import { basename, join } from 'node:path';
const stage = basename(process.argv[1]).slice(0, -3);
const args = process.argv.slice(2);
appendFileSync(process.env.RELEASE_TRACE, JSON.stringify({ stage,
  ld: process.env.LD_LIBRARY_PATH ?? null,
  dyld: process.env.DYLD_FALLBACK_LIBRARY_PATH ?? null }) + '\\n');
if (stage === 'package-brain-product.sh') {
  const output = args[args.indexOf('--output') + 1];
  mkdirSync(output);
  writeFileSync(join(output, 'product-manifest.json'), '{}\\n');
}
if (stage === 'verify-daemon-runtime.sh') {
  if (!existsSync(join(args[args.indexOf('--artifact') + 1], 'product-manifest.json'))) process.exit(81);
}
if (stage === 'verify-brain-product.sh') {
  if (!existsSync(args[args.indexOf('--archive') + 1])) process.exit(82);
}
`;
      for (const script of ['verify-frontend-e2e.sh', 'verify-frontend-artifact.sh',
        'package-brain-product.sh', 'verify-daemon-runtime.sh', 'verify-brain-product.sh']) {
        writeFileSync(join(runRoot, 'scripts', `${script}.js`), stageFixture, { mode: 0o755 });
        symlinkSync(`${script}.js`, join(runRoot, 'scripts', script));
      }
      const run = Bun.spawnSync(['bash', join(runRoot, 'package.sh')], {
        env: { ...process.env, BASH_ENV: bashEnv, RELEASE_ROOT: runRoot,
          RELEASE_TRACE: trace, RELEASE_LIBRARY: libraryDir, RELEASE_GATE_PATHS: join(runRoot, 'gate paths'),
          RELEASE_PRIOR_LD: prior.ld, RELEASE_PRIOR_DYLD: prior.dyld,
          CARGO_ENCODED_RUSTFLAGS: '', RUSTFLAGS: '' },
      });
      expect({ exit: run.exitCode, stderr: run.stderr.toString() }).toEqual({ exit: 0, stderr: '' });
      const ld = prior.ld ? `${libraryDir}:${prior.ld}` : libraryDir;
      const dyld = prior.dyld ? `${libraryDir}:${prior.dyld}` : libraryDir;
      expect(readFileSync(join(runRoot, 'gate paths'), 'utf8')).toBe(`${ld}\n${dyld}\n`);
      const stages = readFileSync(trace, 'utf8').trim().split('\n').map(line => JSON.parse(line));
      expect(stages).toEqual([
        { stage: 'verify-frontend-e2e.sh', ld, dyld },
        ...['verify-frontend-artifact.sh', 'package-brain-product.sh', 'verify-daemon-runtime.sh',
          'verify-brain-product.sh'].map(stage => ({ stage, ld: prior.ld || null, dyld: prior.dyld || null })),
      ]);
    }
  } finally { rmSync(root, { recursive: true, force: true }); }
});

test('native workflow passes an unused product path to the assembler and archives that layout', async () => {
  const { steps } = (await loadWorkflow()).jobs['build-binaries'];
  const assemble = steps.find(step => step.name === 'Assemble complete product');
  const relocate = steps.find(step => step.name === 'Verify daemon runtime relocation in product');
  const archive = steps.find(step => step.name === 'Archive product delivery');
  const root = realpathSync(mkdtempSync(join(tmpdir(), 'bfp native stage ')));
  const runnerTemp = join(root, 'runner temp');
  const stepOutputs = join(root, 'outputs');
  try {
    mkdirSync(runnerTemp);
    mkdirSync(join(root, 'scripts'));
    writeFileSync(join(root, 'scripts/package-brain-product.sh'), [
      '#!/bin/bash',
      'set -eu',
      'while [[ "$1" != --output ]]; do shift 2; done',
      '[[ ! -e "$2" ]] || exit 83',
      'mkdir "$2"',
      'touch "$2/product-manifest.json"',
    ].join('\n'), { mode: 0o755 });
    writeFileSync(join(root, 'scripts/verify-daemon-runtime.sh'), [
      '#!/bin/bash',
      'set -eu',
      '[[ "$1" == --artifact ]]',
      '[[ -f "$2/product-manifest.json" ]]',
    ].join('\n'), { mode: 0o755 });

    const render = (step: WorkflowStep | undefined) => {
      expect(step?.run).toBeDefined();
      return step!.run!
        .replaceAll('${{ runner.temp }}', runnerTemp)
        .replaceAll('${{ matrix.target }}', 'x86_64-unknown-linux-gnu')
        .replaceAll('${{ matrix.artifact_name }}', 'brain-linux-x86_64');
    };
    const env = { ...process.env, RUNNER_TEMP: runnerTemp, GITHUB_OUTPUT: stepOutputs };
    const assembled = Bun.spawnSync(['bash', '-e', '-c', render(assemble)], { cwd: root, env });
    expect({ exit: assembled.exitCode, stderr: assembled.stderr.toString() }).toEqual({ exit: 0, stderr: '' });

    const output = readFileSync(stepOutputs, 'utf8').trim();
    expect(output).toStartWith('product-stage=');
    const productStage = output.slice('product-stage='.length);
    const stagedEnv = { ...env, BRAIN_PRODUCT_STAGE: productStage };
    for (const step of [relocate, archive]) {
      expect(step?.env?.BRAIN_PRODUCT_STAGE).toBe('${{ steps.assemble.outputs.product-stage }}');
      const result = Bun.spawnSync(['bash', '-e', '-c', render(step)], { cwd: root, env: stagedEnv });
      expect({ exit: result.exitCode, stderr: result.stderr.toString() }).toEqual({ exit: 0, stderr: '' });
    }
    const contents = Bun.spawnSync(['tar', '-tzf', 'brain-linux-x86_64.tar.gz'], { cwd: root });
    expect(contents.exitCode).toBe(0);
    expect(contents.stdout.toString()).toContain('brain-dist/product-manifest.json');
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});
