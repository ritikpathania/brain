import { resolveFrontendConfiguration } from './configuration.js';
import { readCompiledIdentity } from './buildIdentity.js';

async function main(): Promise<void> {
  if (Bun.argv.slice(2).includes('--build-identity')) {
    const identity = readCompiledIdentity();
    if (identity === null) throw new Error('build identity is available only in the compiled frontend; run bun run build');
    console.log(JSON.stringify(identity));
    return;
  }
  const configuration = resolveFrontendConfiguration(Bun.argv.slice(2));
  const [{ BrainFrontendBootstrap }, { renderBrainTerminal }, { BrainFrontendController }] = await Promise.all([
    import('../app/bootstrap.js'), import('../presentation/terminal/renderer.js'), import('../state/interactions.js'),
  ]);
  const frontend = new BrainFrontendBootstrap(configuration);
  const controller = BrainFrontendController.fromBootstrap(frontend);
  const terminal = renderBrainTerminal(controller);
  try {
    if (await controller.start() === 'ready') await controller.openSessionPicker();
    await terminal.waitUntilExit();
  } finally {
    terminal.unmount();
    await frontend.shutdown();
  }
}

void main().catch(error => {
  console.error(error instanceof Error ? error.message : String(error));
  process.exitCode = 1;
});
