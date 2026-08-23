import { describe, expect, test } from 'bun:test';
import { getProductionDeps, productionDeps, type QueryDeps } from '../../contracts/query.js';

describe('contracts/query', () => {
  test('productionDeps exposes an async-generator callModel backed by the Brain adapter', () => {
    expect(typeof productionDeps.callModel).toBe('function');
    expect(productionDeps.callModel.constructor.name).toBe('AsyncGeneratorFunction');
  });

  test('getProductionDeps caches a single instance', () => {
    expect(getProductionDeps()).toBe(getProductionDeps());
  });

  test('QueryDeps shape matches the seam the REPL loop will consume', () => {
    const deps: QueryDeps = productionDeps;
    expect(deps.callModel).toBeDefined();
  });
});
