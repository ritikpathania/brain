/**
 * Inc 36: Unit tests for ContextOverlayView component.
 */
import { describe, expect, test } from 'bun:test';
import * as React from 'react';
import { PALETTES } from '../../../state/palettes.js';
import { ContextOverlayView } from '../../../ui/overlays/ContextOverlayView.js';
import type { ContextTelemetry } from '../../../client/BrainBackendClient.js';

function textOf(el: React.ReactElement): string {
  const walk = (node: React.ReactNode): string => {
    if (node === null || node === undefined || typeof node === 'boolean') return '';
    if (typeof node === 'string' || typeof node === 'number') return String(node);
    if (Array.isArray(node)) return node.map(walk).join('');
    const el2 = node as React.ReactElement;
    let text = '';
    if (el2.props && typeof el2.props === 'object') {
      if (typeof (el2.props as { title?: string }).title === 'string') {
        text += (el2.props as { title?: string }).title + ' ';
      }
      if (typeof (el2.props as { subtitle?: string }).subtitle === 'string') {
        text += (el2.props as { subtitle?: string }).subtitle + ' ';
      }
      if ('children' in el2.props) {
        text += walk((el2.props as { children?: React.ReactNode }).children);
      }
    }
    return text;
  };
  return walk(el);
}

const tokens = PALETTES.dark;

const POPULATED_TELEMETRY: ContextTelemetry = {
  generationId: 'gen-999',
  sessionId: '01M1234567890ABCDEF',
  retrievalEpochId: 'ep-epoch-quantum-42',
  candidatesRetrieved: 10,
  memoriesAssembled: 4,
  contextTokensUsed: 312,
  assemblyLatencyMs: 7,
  totalDurationMs: 540,
  finishReason: 'stop',
};

const ZERO_MEMORY_TELEMETRY: ContextTelemetry = {
  generationId: 'gen-000',
  sessionId: '01M1234567890ABCDEF',
  retrievalEpochId: 'ep-epoch-empty-00',
  candidatesRetrieved: 5,
  memoriesAssembled: 0,
  contextTokensUsed: 0,
  assemblyLatencyMs: 3,
  totalDurationMs: 210,
  finishReason: 'stop',
};

describe('ContextOverlayView (Inc 36)', () => {
  test('renders populated context provenance, token counts, and latency', () => {
    const out = textOf(ContextOverlayView({ telemetry: POPULATED_TELEMETRY, tokens }));
    expect(out).toContain('Context Provenance');
    expect(out).toContain('ep-epoch-quantum-42');
    expect(out).toContain('Candidates evaluated: 10');
    expect(out).toContain('Memories assembled into prompt: 4');
    expect(out).toContain('Context tokens injected: 312 tokens');
    expect(out).toContain('Assembly latency: 7ms');
    expect(out).toContain('Total turn duration: 540ms');
    expect(out).toContain('Finish reason: stop');
  });

  test('renders zero-memory notice when retrieval candidates did not meet threshold', () => {
    const out = textOf(ContextOverlayView({ telemetry: ZERO_MEMORY_TELEMETRY, tokens }));
    expect(out).toContain('Context Provenance');
    expect(out).toContain('Candidates evaluated: 5');
    expect(out).toContain('Memories assembled into prompt: 0');
    expect(out).toContain('Context tokens injected: 0 tokens');
    expect(out).toContain('0 memories met the threshold');
  });

  test('renders unavailable notice when telemetry is missing/undefined', () => {
    const out = textOf(ContextOverlayView({ telemetry: undefined, tokens }));
    expect(out).toContain('Context Provenance');
    expect(out).toContain('No context retrieval telemetry recorded for the active turn');
    expect(out).toContain('Context provenance is captured when a prompt is submitted');
  });
});
