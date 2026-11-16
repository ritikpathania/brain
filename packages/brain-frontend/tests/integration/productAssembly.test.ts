import { expect, test } from 'bun:test';
import { resolve } from 'node:path';

const root = resolve(import.meta.dir, '../../../..');
const python = process.env.PYO3_PYTHON ?? resolve(root, 'daemon/.venv/bin/python');

test('product manifest and licenses unit test suite passes with python verifier', async () => {
  const runManifest = Bun.spawn(
    [python, '-m', 'unittest', 'scripts/release/test_product_manifest.py'],
    { cwd: root, stdout: 'pipe', stderr: 'pipe' }
  );
  const [mOut, mErr, mCode] = await Promise.all([
    new Response(runManifest.stdout).text(),
    new Response(runManifest.stderr).text(),
    runManifest.exited,
  ]);
  if (mCode !== 0) console.error(mOut, mErr);
  expect(mCode).toBe(0);

  const runLicenses = Bun.spawn(
    [python, '-m', 'unittest', 'scripts/release/test_licenses.py'],
    { cwd: root, stdout: 'pipe', stderr: 'pipe' }
  );
  const [lOut, lErr, lCode] = await Promise.all([
    new Response(runLicenses.stdout).text(),
    new Response(runLicenses.stderr).text(),
    runLicenses.exited,
  ]);
  if (lCode !== 0) console.error(lOut, lErr);
  expect(lCode).toBe(0);
}, 20_000);

test('package-brain-product.sh refuses existing output directory and requires arguments', () => {
  const pkgScript = resolve(root, 'scripts/package-brain-product.sh');

  // Missing arguments
  const missingArgs = Bun.spawnSync(['bash', pkgScript], {
    cwd: root,
  });
  expect(missingArgs.exitCode).not.toBe(0);

  // Existing output directory
  const existingDir = Bun.spawnSync(
    [
      'bash',
      pkgScript,
      '--cli',
      resolve(root, 'target/release/brain'),
      '--daemon',
      resolve(root, 'target/release/brain-daemon'),
      '--frontend',
      resolve(root, 'packages/brain-frontend/dist/brain-frontend'),
      '--output',
      root,
      '--target',
      'aarch64-apple-darwin',
    ],
    { cwd: root }
  );
  expect(existingDir.exitCode).not.toBe(0);
  expect(existingDir.stderr.toString()).toContain('error:');
});
