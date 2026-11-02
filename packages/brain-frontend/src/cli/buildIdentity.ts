import type { SourceIdentity } from '../../scripts/release/sourceIdentity.js';

export type BuildIdentity = Readonly<{
  schema_version: 1;
  identity: 'brain-frontend';
  package: '@brain/frontend';
  frontend_version: string;
  brain_version: string;
  source: SourceIdentity;
  target_triple: string;
  runtime_model: 'bun-standalone';
  bun_version: '1.4.4';
  lockfile_sha256: string;
}>;

declare const __BRAIN_COMPILED_IDENTITY__: BuildIdentity | undefined;

/** Only the compiler can set this literal. Source execution has no release identity. */
export function readCompiledIdentity(): BuildIdentity | null {
  if (typeof __BRAIN_COMPILED_IDENTITY__ === 'undefined') return null;
  const identity = __BRAIN_COMPILED_IDENTITY__;
  if (identity === undefined) return null;
  Object.freeze(identity.source);
  return Object.freeze(identity);
}
