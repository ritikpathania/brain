/**
 * Inc 36: /context overlay view — displays active turn context retrieval provenance and token telemetry.
 */
import * as React from 'react';
import { Box, Text } from '../../compat/index.js';
import type { BrainTokens } from '../../state/palettes.js';
import type { ContextTelemetry } from '../../client/BrainBackendClient.js';
import { ModalFrame } from './ModalFrame.js';

export function ContextOverlayView(props: {
  telemetry: ContextTelemetry | undefined;
  activeSessionId?: string;
  tokens: BrainTokens;
}): React.ReactElement {
  const { telemetry, activeSessionId, tokens } = props;

  return (
    <ModalFrame
      title="Context Provenance"
      subtitle="Live memory context augmentation and token telemetry for the active turn"
      footerHints="Enter / Esc to close"
      width={80}
    >
      {!telemetry ? (
        <Box flexDirection="column" gap={1}>
          <Text color={tokens.warning}>No context retrieval telemetry recorded for the active turn.</Text>
          <Text dimColor>Context provenance is captured when a prompt is submitted and augmented by the memory orchestrator.</Text>
        </Box>
      ) : (
        <Box flexDirection="column" gap={1}>
          <Box flexDirection="row">
            <Text bold>Retrieval Epoch: </Text>
            <Text color={tokens.accent}>{telemetry.retrievalEpochId ?? 'epoch-fallback'}</Text>
          </Box>

          <Box flexDirection="column">
            <Text bold color={tokens.brand}>Memory Augmentation:</Text>
            <Text>
              {'  '}<Text color={tokens.subtle}>• Candidates evaluated: </Text>
              <Text bold>{telemetry.candidatesRetrieved}</Text>
            </Text>
            <Text>
              {'  '}<Text color={tokens.subtle}>• Memories assembled into prompt: </Text>
              <Text bold color={telemetry.memoriesAssembled > 0 ? tokens.success : tokens.muted}>
                {telemetry.memoriesAssembled}
              </Text>
            </Text>
            <Text>
              {'  '}<Text color={tokens.subtle}>• Context tokens injected: </Text>
              <Text bold>{telemetry.contextTokensUsed} tokens</Text>
            </Text>
            {telemetry.memoriesAssembled === 0 && (
              <Text dimColor>
                {'  '}⎿ Note: Retrieval evaluated candidates, but 0 memories met the threshold for prompt assembly.
              </Text>
            )}
          </Box>

          <Box flexDirection="column">
            <Text bold color={tokens.brand}>Latency & Execution:</Text>
            <Text>
              {'  '}<Text color={tokens.subtle}>• Assembly latency: </Text>
              <Text>{telemetry.assemblyLatencyMs !== undefined ? `${telemetry.assemblyLatencyMs}ms` : 'N/A'}</Text>
            </Text>
            {telemetry.totalDurationMs !== undefined && (
              <Text>
                {'  '}<Text color={tokens.subtle}>• Total turn duration: </Text>
                <Text>{telemetry.totalDurationMs}ms</Text>
              </Text>
            )}
            {telemetry.finishReason && (
              <Text>
                {'  '}<Text color={tokens.subtle}>• Finish reason: </Text>
                <Text color={tokens.muted}>{telemetry.finishReason}</Text>
              </Text>
            )}
          </Box>

          {telemetry.sessionId || activeSessionId ? (
            <Text dimColor>
              Session: {(telemetry.sessionId ?? activeSessionId)?.slice(0, 16)}…
            </Text>
          ) : null}
        </Box>
      )}
    </ModalFrame>
  );
}
