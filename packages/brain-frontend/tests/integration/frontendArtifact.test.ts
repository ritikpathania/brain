import { expect, test } from 'bun:test';
import { resolve } from 'node:path';

const root = resolve(import.meta.dir, '../../../..');
const python = process.env.PYO3_PYTHON ?? resolve(root, 'daemon/.venv/bin/python');
test('frontend artifact shell validates schema/hash/diagnostic and propagates failure', async () => {
  const run = Bun.spawn([python, '-m', 'unittest', 'discover', '-s', 'scripts/release', '-p', 'test_artifact_manifest.py'],
    { cwd: root, stdout: 'pipe', stderr: 'pipe' });
  const [output, errors, code] = await Promise.all([new Response(run.stdout).text(), new Response(run.stderr).text(), run.exited]);
  if (code !== 0) console.error(output, errors);
  expect(code).toBe(0);
  const invalid = Bun.spawnSync(['bash', resolve(root, 'scripts/verify-frontend-artifact.sh'),
    '--artifact', '/nonexistent/brain-frontend', '--brain-version', '1.1.0', '--target', 'aarch64-apple-darwin'],
    { cwd: root, env: { ...process.env, PYO3_PYTHON: python } });
  expect(invalid.exitCode).not.toBe(0);
  expect(invalid.stderr.toString()).toContain('error:');
}, 15_000);
