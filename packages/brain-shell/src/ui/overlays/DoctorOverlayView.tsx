/** /doctor overlay body: local probe results as a read-only report. Pure view. */
import * as React from 'react';
import { Box, Text } from '../../compat/index.js';
import type { BrainTokens } from '../../state/palettes.js';
import type { EngineDiagnosticReport } from '../../adapter/doctorProbe.js';
import { ModalFrame } from './ModalFrame.js';

export function DoctorOverlayView(props: {
  loading: boolean;
  report: EngineDiagnosticReport | null;
  tokens: BrainTokens;
}): React.ReactElement {
  void props.tokens; // reserved: rows gain semantic colors in later increments
  return (
    <ModalFrame
      title="Brain System Doctor"
      subtitle="Subsystem health probes, IPC socket latency, and SQLite storage verification"
      footerHints="Enter / Esc to dismiss"
      width={80}
    >
      {props.loading ? (
        <Text color="yellow">Running diagnostic health probes…</Text>
      ) : props.report === null ? (
        <Text color="red">Failed to collect diagnostic signals.</Text>
      ) : (
        <Box flexDirection="column" gap={1}>
          <Box flexDirection="row">
            <Text bold>Overall System Health: </Text>
            <Text color={props.report.overallHealthy ? 'green' : 'red'} bold>
              {props.report.overallHealthy ? '● HEALTHY' : '▲ DEGRADED / UNHEALTHY'}
            </Text>
          </Box>
          <Box flexDirection="column">
            <Text bold color="cyan">Observable Subsystem Probes:</Text>
            {props.report.subsystems.map((sub, idx) => (
              <Box key={idx} flexDirection="column">
                <Text>
                  <Text color={sub.status === 'healthy' ? 'green' : 'red'}>
                    {sub.status === 'healthy' ? '✔' : '✖'}
                  </Text>
                  {' '}{sub.subsystem}
                  {sub.latencyMs !== undefined ? ` (${sub.latencyMs}ms)` : ''}
                </Text>
                <Text dimColor>{'⎿ '}{sub.message}</Text>
              </Box>
            ))}
          </Box>
          <Box flexDirection="column">
            <Text bold color="cyan">Remediation Actions:</Text>
            {props.report.overallHealthy ? (
              <Text dimColor>{'⎿ '}No remediation required. All local subsystems operational.</Text>
            ) : (
              <Text color="yellow">⎿ Daemon unreachable. Run `brain daemon start` or `make dev` to start the service.</Text>
            )}
          </Box>
        </Box>
      )}
    </ModalFrame>
  );
}
