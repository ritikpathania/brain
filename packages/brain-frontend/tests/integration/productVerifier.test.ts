import { expect, test } from 'bun:test';
import { resolve } from 'node:path';

const root = resolve(import.meta.dir, '../../../..');
const python = process.env.PYO3_PYTHON ?? resolve(root, 'daemon/.venv/bin/python');

test('product smoke unit tests pass with python verifier', async () => {
  const runSmoke = Bun.spawn(
    [python, '-m', 'unittest', 'scripts/release/test_product_smoke.py'],
    { cwd: root, stdout: 'pipe', stderr: 'pipe' }
  );
  const [sOut, sErr, sCode] = await Promise.all([
    new Response(runSmoke.stdout).text(),
    new Response(runSmoke.stderr).text(),
    runSmoke.exited,
  ]);
  if (sCode !== 0) console.error(sOut, sErr);
  expect(sCode).toBe(0);
}, 20_000);

test('verify-brain-product.sh requires valid arguments and fails clearly on invalid inputs', () => {
  const verifyScript = resolve(root, 'scripts/verify-brain-product.sh');

  // Missing arguments
  const missingArgs = Bun.spawnSync(['bash', verifyScript], { cwd: root });
  expect(missingArgs.exitCode).not.toBe(0);

  // Missing artifact directory
  const missingArtifact = Bun.spawnSync(
    [
      'bash',
      verifyScript,
      '--artifact',
      '/nonexistent/product',
      '--target',
      'aarch64-apple-darwin',
    ],
    { cwd: root, env: { ...process.env, PYO3_PYTHON: python } }
  );
  expect(missingArtifact.exitCode).not.toBe(0);
  expect(missingArtifact.stderr.toString()).toContain('error:');
});
