import { describe, expect, it } from 'bun:test';
import { DoctorProbe, formatUptime } from '../../adapter/doctorProbe.js';
import type {
  BrainBackendClient,
  SystemStatusOutput,
} from '../../client/BrainBackendClient.js';

class MockBackendForDoctor implements Partial<BrainBackendClient> {
  constructor(
    public mockStatus: SystemStatusOutput = {
      status: 'healthy',
      version: '1.1.0',
      uptimeSeconds: 125,
      sqliteHealthy: true,
      totalSessions: 3,
      totalNodes: 15,
      totalEdges: 8,
      activeModelsCount: 2,
    },
    public shouldFail = false,
  ) {}

  async getSystemStatus(): Promise<SystemStatusOutput> {
    if (this.shouldFail) {
      throw new Error('RPC query failed');
    }
    return this.mockStatus;
  }
}

describe('Increment 45: Authoritative Daemon Diagnostics & Subsystem Telemetry', () => {
  describe('formatUptime helper', () => {
    it('formats seconds correctly', () => {
      expect(formatUptime(0)).toBe('0s');
      expect(formatUptime(45)).toBe('45s');
    });

    it('formats minutes and seconds correctly', () => {
      expect(formatUptime(60)).toBe('1m 0s');
      expect(formatUptime(125)).toBe('2m 5s');
      expect(formatUptime(3599)).toBe('59m 59s');
    });

    it('formats hours and minutes correctly', () => {
      expect(formatUptime(3600)).toBe('1h 0m');
      expect(formatUptime(7320)).toBe('2h 2m');
    });
  });

  describe('DoctorProbe authoritative telemetry', () => {
    it('queries client.getSystemStatus() and constructs authoritative subsystem probes', async () => {
      const mock = new MockBackendForDoctor();
      // Use dummy socket path that checkUdsSocket might fail or skip, but mock checkUdsSocket via probe
      const probe = new DoctorProbe('/tmp/nonexistent-sock.sock', mock as unknown as BrainBackendClient);

      // Spy or override checkUdsSocket to simulate connected socket
      (probe as any).checkUdsSocket = async () => ({
        subsystem: 'UDS Daemon Socket',
        status: 'healthy',
        latencyMs: 3,
        message: 'Socket responding (3ms)',
      });

      const report = await probe.runDiagnostics();

      expect(report.overallHealthy).toBe(true);
      expect(report.subsystems.length).toBe(3);

      const daemonSub = report.subsystems.find((s) => s.subsystem.includes('UDS Daemon'));
      expect(daemonSub).toBeDefined();
      expect(daemonSub?.status).toBe('healthy');
      expect(daemonSub?.message).toContain('Daemon v1.1.0 online');
      expect(daemonSub?.message).toContain('Uptime: 2m 5s');
      expect(daemonSub?.latencyMs).toBe(3);

      const sqliteSub = report.subsystems.find((s) => s.subsystem === 'SQLite Database & Graph');
      expect(sqliteSub).toBeDefined();
      expect(sqliteSub?.status).toBe('healthy');
      expect(sqliteSub?.message).toContain('3 sessions · 15 concepts · 8 relations');

      const modelSub = report.subsystems.find((s) => s.subsystem === 'Model Gateway & Inference');
      expect(modelSub).toBeDefined();
      expect(modelSub?.status).toBe('healthy');
      expect(modelSub?.message).toContain('2 active models registered');
    });

    it('marks report as degraded/unhealthy when SQLite or status is degraded', async () => {
      const mock = new MockBackendForDoctor({
        status: 'degraded',
        version: '1.1.0',
        uptimeSeconds: 500,
        sqliteHealthy: false,
        totalSessions: 0,
        totalNodes: 0,
        totalEdges: 0,
        activeModelsCount: 1,
      });

      const probe = new DoctorProbe('/tmp/dummy.sock', mock as unknown as BrainBackendClient);
      (probe as any).checkUdsSocket = async () => ({
        subsystem: 'UDS Daemon Socket',
        status: 'healthy',
        latencyMs: 5,
        message: 'Socket responding (5ms)',
      });

      const report = await probe.runDiagnostics();

      expect(report.overallHealthy).toBe(false);

      const sqliteSub = report.subsystems.find((s) => s.subsystem === 'SQLite Database & Graph');
      expect(sqliteSub?.status).toBe('degraded');
      expect(sqliteSub?.message).toContain('SQLite database connection degraded');
    });

    it('falls back to local filesystem checks when getSystemStatus fails', async () => {
      const mock = new MockBackendForDoctor(undefined, true);
      const probe = new DoctorProbe('/tmp/dummy.sock', mock as unknown as BrainBackendClient);
      (probe as any).checkUdsSocket = async () => ({
        subsystem: 'UDS Daemon Socket',
        status: 'healthy',
        latencyMs: 2,
        message: 'Socket responding (2ms)',
      });

      const report = await probe.runDiagnostics();

      // Fallback generates raw socket, local storage, and memory engine probes
      expect(report.subsystems.some((s) => s.subsystem === 'SQLite WAL Storage')).toBe(true);
      expect(report.subsystems.some((s) => s.subsystem === 'Memory Engine (STM/LTM)')).toBe(true);
    });
  });
});
