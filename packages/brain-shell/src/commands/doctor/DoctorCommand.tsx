import React, { useState, useEffect } from 'react';
import { Box, Text, useInput } from '../../compat/ink.js';
import { useTerminalSize } from '../../compat/hooks.js';
import { DoctorProbe, type EngineDiagnosticReport } from '../../adapter/doctorProbe.js';
import { BrainModal } from '../../components/BrainModal.js';

export interface DoctorCommandProps {
  onDone: (result?: string, options?: { display?: 'system' | 'user' }) => void;
  probe?: DoctorProbe;
}

export const DoctorCommand: React.FC<DoctorCommandProps> = ({
  onDone,
  probe = new DoctorProbe(),
}) => {
  const [report, setReport] = useState<EngineDiagnosticReport | null>(null);
  const [isLoading, setIsLoading] = useState<boolean>(true);
  const { columns } = useTerminalSize();

  useEffect(() => {
    let mounted = true;
    probe
      .runDiagnostics()
      .then((rep) => {
        if (mounted) {
          setReport(rep);
          setIsLoading(false);
        }
      })
      .catch(() => {
        if (mounted) {
          setIsLoading(false);
        }
      });
    return () => {
      mounted = false;
    };
  }, [probe]);

  useInput((input, key) => {
    if (key.escape || key.return) {
      onDone('Completed system diagnostics', { display: 'system' });
    }
  });

  return (
    <BrainModal
      title="Brain System Doctor"
      subtitle="Subsystem health probes, IPC socket latency, and SQLite storage verification"
      footerHints="Enter / Esc to dismiss"
      onDismiss={() => onDone('Completed system diagnostics', { display: 'system' })}
      width={Math.min(80, columns)}
    >
      <Box flexDirection="column" gap={1}>
        {isLoading ? (
          <Text color="yellow">Running diagnostic health probes…</Text>
        ) : !report ? (
          <Text color="red">Failed to collect diagnostic signals.</Text>
        ) : (
          <Box flexDirection="column">
            <Box flexDirection="row" marginBottom={1}>
              <Text bold>Overall System Health: </Text>
              <Text color={report.overallHealthy ? 'green' : 'red'} bold>
                {report.overallHealthy ? '● HEALTHY' : '▲ DEGRADED / UNHEALTHY'}
              </Text>
            </Box>

            <Box flexDirection="column">
              <Text bold color="cyan">
                Observable Subsystem Probes:
              </Text>
              {report.subsystems.map((sub, idx) => {
                const isHealthy = sub.status === 'healthy';
                return (
                  <Box key={idx} flexDirection="column" marginTop={1} paddingLeft={1}>
                    <Box flexDirection="row">
                      <Text color={isHealthy ? 'green' : 'red'}>
                        {isHealthy ? '✔' : '✖'}
                      </Text>
                      <Text bold color={isHealthy ? undefined : 'red'}>
                        {' '}{sub.subsystem}:
                      </Text>
                      {sub.latencyMs !== undefined && (
                        <Text dimColor> ({sub.latencyMs}ms)</Text>
                      )}
                    </Box>
                    <Box paddingLeft={2}>
                      <Text dimColor>
                        {'⎿ '}{sub.message}
                      </Text>
                    </Box>
                  </Box>
                );
              })}
            </Box>

            <Box flexDirection="column" marginTop={1}>
              <Text bold color="cyan">
                Remediation Actions:
              </Text>
              <Box paddingLeft={1}>
                {report.overallHealthy ? (
                  <Text dimColor>⎿ No remediation required. All local subsystems operational.</Text>
                ) : (
                  <Text color="yellow">
                    ⎿ Daemon unreachable. Run `brain daemon start` or `make dev` to start the service.
                  </Text>
                )}
              </Box>
            </Box>
          </Box>
        )}
      </Box>
    </BrainModal>
  );
};

export default DoctorCommand;
