export type FrontendConfiguration = Readonly<{
  socketPath: string;
  frontendVersion: string;
  frontendBuildId: string | null;
}>;

export class FrontendConfigurationError extends Error {
  public constructor(message: string) {
    super(`invalid_frontend_configuration: ${message}`);
    this.name = 'FrontendConfigurationError';
  }
}

/** Resolves only explicit Brain runtime inputs; it never reads Claude config. */
export function resolveFrontendConfiguration(
  argumentsList: readonly string[],
  environment: Readonly<Record<string, string | undefined>> = process.env,
  compiledIdentity: BuildIdentity | null = readCompiledIdentity(),
): FrontendConfiguration {
  const socketPath = option(argumentsList, '--socket') ?? environment.BRAIN_SOCKET_PATH;
  if (socketPath === undefined || socketPath.trim().length === 0) {
    throw new FrontendConfigurationError('provide --socket <path> or BRAIN_SOCKET_PATH');
  }
  const frontendVersion = compiledIdentity?.frontend_version ?? option(argumentsList, '--frontend-version') ?? environment.BRAIN_FRONTEND_VERSION ?? '0.1.0';
  if (frontendVersion.trim().length === 0) {
    throw new FrontendConfigurationError('frontend version must not be empty');
  }
  const frontendBuildId = compiledIdentity?.source.fingerprint_sha256 ?? option(argumentsList, '--frontend-build-id') ?? environment.BRAIN_FRONTEND_BUILD_ID ?? null;
  return Object.freeze({ socketPath, frontendVersion, frontendBuildId });
}

function option(argumentsList: readonly string[], name: string): string | undefined {
  const index = argumentsList.indexOf(name);
  if (index === -1) return undefined;
  const value = argumentsList[index + 1];
  if (value === undefined || value.startsWith('--')) {
    throw new FrontendConfigurationError(`${name} requires a value`);
  }
  return value;
}
import { readCompiledIdentity, type BuildIdentity } from './buildIdentity.js';
