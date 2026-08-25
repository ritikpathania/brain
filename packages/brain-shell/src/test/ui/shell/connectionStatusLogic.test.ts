import { describe, it, expect } from 'bun:test';
import { connectionStatusText } from '../../../ui/shell/connectionStatusLogic.js';

describe('connectionStatusText', () => {
  it('hides when connected or unknown', () => {
    expect(connectionStatusText(undefined)).toBeNull();
    expect(connectionStatusText({ status: 'connected' })).toBeNull();
  });

  it('reports the attempt count while reconnecting', () => {
    expect(connectionStatusText({ status: 'reconnecting', attempt: 1 })).toBe(
      'reconnecting (attempt 1)',
    );
    expect(connectionStatusText({ status: 'reconnecting', attempt: 7 })).toBe(
      'reconnecting (attempt 7)',
    );
  });
});
