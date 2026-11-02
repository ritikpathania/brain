import { expect, test } from 'bun:test';
import { resolve } from 'node:path';

type WorkflowStep = {
  name?: string;
  run?: string;
  if?: unknown;
  'continue-on-error'?: unknown;
  uses?: string;
  with?: Record<string, unknown>;
  shell?: string;
};

type WorkflowJob = {
  name?: string;
  needs?: string | string[];
  runs_on?: string;
  strategy?: {
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
  jobs: {
    'release-quality': WorkflowJob;
    'build-binaries': WorkflowJob;
    'publish-release': WorkflowJob;
  };
};

test('release workflow defines unified product matrix, blocking quality, and product verification without external authorization guards', async () => {
  const workflowPath = resolve(import.meta.dir, '../../../../.github/workflows/release.yml');
  const workflow = Bun.YAML.parse(await Bun.file(workflowPath).text()) as ReleaseWorkflow;

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

  // Ensure Bun 1.4.4 and Python 3.15.2 are configured
  const bunStep = buildJob.steps.find((s) => s.uses?.startsWith('oven-sh/setup-bun'));
  expect(bunStep).toBeDefined();
  expect(bunStep?.with?.['bun-version']).toBe('1.4.4');

  const pyStep = buildJob.steps.find((s) => s.uses?.startsWith('actions/setup-python'));
  expect(pyStep).toBeDefined();
  expect(pyStep?.with?.['python-version']).toBe('3.15.2');

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
