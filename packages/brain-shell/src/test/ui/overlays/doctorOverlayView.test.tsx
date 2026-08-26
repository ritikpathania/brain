import { describe, expect, test } from 'bun:test';
import * as React from 'react';
import { PALETTES } from '../../../state/palettes.js';
import { DoctorOverlayView } from '../../../ui/overlays/DoctorOverlayView.js';
import type { EngineDiagnosticReport } from '../../../adapter/doctorProbe.js';

function textOf(el: React.ReactElement): string {
  const walk = (node: React.ReactNode): string => {
    if (node === null || node === undefined || typeof node === 'boolean') return '';
    if (typeof node === 'string' || typeof node === 'number') return String(node);
    if (Array.isArray(node)) return node.map(walk).join('');
    const el2 = node as React.ReactElement;
    if (el2.props && typeof el2.props === 'object' && 'children' in el2.props) {
      return walk((el2.props as { children?: React.ReactNode }).children);
    }
    return '';
  };
  return walk(el);
}

const tokens = PALETTES.dark;

const HEALTHY: EngineDiagnosticReport = {
  timestamp: '2026-08-26T00:00:00Z',
  overallHealthy: true,
  socketPath: '/tmp/x.sock',
  subsystems: [
    { subsystem: 'UDS Daemon Socket', status: 'healthy', latencyMs: 3, message: 'responding' },
    { subsystem: 'SQLite WAL Storage', status: 'healthy', message: 'initialized' },
  ],
};

const DEGRADED: EngineDiagnosticReport = {
  timestamp: '2026-08-26T00:00:00Z',
  overallHealthy: false,
  socketPath: '/tmp/x.sock',
  subsystems: [
    { subsystem: 'UDS Daemon Socket', status: 'unhealthy', message: 'timed out' },
  ],
};

describe('DoctorOverlayView (Inc 21)', () => {
  test('healthy banner, subsystem rows, latency, remediation-none', () => {
    const out = textOf(DoctorOverlayView({ loading: false, report: HEALTHY, tokens }));
    expect(out).toContain('HEALTHY');
    expect(out).toContain('UDS Daemon Socket');
    expect(out).toContain('(3ms)');
    expect(out).toContain('No remediation required');
  });

  test('degraded banner, ✖ row, start-daemon hint', () => {
    const out = textOf(DoctorOverlayView({ loading: false, report: DEGRADED, tokens }));
    expect(out).toContain('DEGRADED');
    expect(out).toContain('✖');
    expect(out).toContain('Daemon unreachable');
  });

  test('loading and failure states', () => {
    expect(textOf(DoctorOverlayView({ loading: true, report: null, tokens }))).toContain(
      'Running diagnostic health probes',
    );
    expect(textOf(DoctorOverlayView({ loading: false, report: null, tokens }))).toContain(
      'Failed to collect diagnostic signals',
    );
  });
});
