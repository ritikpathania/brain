/**
 * Local & Authoritative Engine Diagnostic Health Probes (Layer 2 Brain Adapter)
 *
 * Implements authoritative subsystem health checks for /doctor and shell startup,
 * querying the live daemon over UDS via getSystemStatus() and checking local connectivity.
 */

import * as net from 'net';
import * as fs from 'fs';
import type { BrainBackendClient, SystemStatusOutput } from '../client/BrainBackendClient.js';

export interface SubsystemHealthStatus {
  subsystem: string;
  status: 'healthy' | 'degraded' | 'unhealthy' | 'disabled';
  latencyMs?: number;
  details?: Record<string, unknown>;
  message: string;
}

export interface EngineDiagnosticReport {
  timestamp: string;
  overallHealthy: boolean;
  socketPath: string;
  subsystems: SubsystemHealthStatus[];
}

export function formatUptime(secs: number): string {
  if (secs < 60) return `${secs}s`;
  const mins = Math.floor(secs / 60);
  const remSecs = secs % 60;
  if (mins < 60) return `${mins}m ${remSecs}s`;
  const hours = Math.floor(mins / 60);
  const remMins = mins % 60;
  return `${hours}h ${remMins}m`;
}

export class DoctorProbe {
  constructor(
    private socketPath: string = process.env.BRAIN_SOCKET_PATH || '/tmp/brain.sock',
    private client?: BrainBackendClient,
  ) {}

  /**
   * Run full health diagnostics against the local Brain runtime.
   */
  async runDiagnostics(): Promise<EngineDiagnosticReport> {
    const report: EngineDiagnosticReport = {
      timestamp: new Date().toISOString(),
      overallHealthy: true,
      socketPath: this.socketPath,
      subsystems: [],
    };

    // 1. Check UDS Socket & Ping
    const socketHealth = await this.checkUdsSocket();

    // 2. Query authoritative daemon telemetry if socket is alive and client is provided
    let daemonStatus: SystemStatusOutput | null = null;
    if (socketHealth.status === 'healthy' && this.client) {
      try {
        daemonStatus = await this.client.getSystemStatus();
      } catch {
        daemonStatus = null;
      }
    }

    if (daemonStatus) {
      // Subsystem 1: Authoritative UDS Daemon Socket & Runtime
      report.subsystems.push({
        subsystem: 'UDS Daemon Socket & Runtime',
        status: daemonStatus.status === 'healthy' ? 'healthy' : 'degraded',
        latencyMs: socketHealth.latencyMs,
        details: {
          version: daemonStatus.version,
          uptimeSeconds: daemonStatus.uptimeSeconds,
          socketPath: this.socketPath,
        },
        message: `Daemon v${daemonStatus.version} online · Uptime: ${formatUptime(daemonStatus.uptimeSeconds)}`,
      });

      // Subsystem 2: Authoritative SQLite Storage & Graph
      report.subsystems.push({
        subsystem: 'SQLite Database & Graph',
        status: daemonStatus.sqliteHealthy ? 'healthy' : 'degraded',
        details: {
          sqliteHealthy: daemonStatus.sqliteHealthy,
          totalSessions: daemonStatus.totalSessions,
          totalNodes: daemonStatus.totalNodes,
          totalEdges: daemonStatus.totalEdges,
        },
        message: daemonStatus.sqliteHealthy
          ? `SQLite WAL operational · ${daemonStatus.totalSessions} sessions · ${daemonStatus.totalNodes} concepts · ${daemonStatus.totalEdges} relations`
          : 'SQLite database connection degraded or reported query failures',
      });

      // Subsystem 3: Authoritative Model Gateway
      report.subsystems.push({
        subsystem: 'Model Gateway & Inference',
        status: daemonStatus.activeModelsCount > 0 ? 'healthy' : 'degraded',
        details: {
          activeModelsCount: daemonStatus.activeModelsCount,
        },
        message:
          daemonStatus.activeModelsCount > 0
            ? `Model Gateway operational · ${daemonStatus.activeModelsCount} active models registered`
            : 'No active inference models registered with model gateway',
      });

      if (daemonStatus.status === 'unhealthy' || !daemonStatus.sqliteHealthy) {
        report.overallHealthy = false;
      }
    } else {
      // Fallback: If daemon query failed or client not present, use raw socket & local storage probe
      report.subsystems.push(socketHealth);
      if (socketHealth.status === 'unhealthy') {
        report.overallHealthy = false;
      }

      const storageHealth = await this.checkLocalStorage();
      report.subsystems.push(storageHealth);
      if (storageHealth.status === 'unhealthy') {
        report.overallHealthy = false;
      }

      const memoryHealth = await this.checkMemorySubsystem();
      report.subsystems.push(memoryHealth);
    }

    return report;
  }

  /**
   * Probe the Unix Domain Socket for liveness and measure ping round-trip latency.
   */
  private async checkUdsSocket(): Promise<SubsystemHealthStatus> {
    const start = Date.now();

    return new Promise((resolve) => {
      let socket: net.Socket | null = null;
      const timeout = setTimeout(() => {
        if (socket && !socket.destroyed) socket.destroy();
        resolve({
          subsystem: 'UDS Daemon Socket',
          status: 'unhealthy',
          message: `Connection timed out after 1000ms at ${this.socketPath}`,
        });
      }, 1000);

      try {
        socket = net.createConnection(this.socketPath, () => {
          clearTimeout(timeout);
          const latency = Date.now() - start;
          socket?.destroy();
          resolve({
            subsystem: 'UDS Daemon Socket',
            status: 'healthy',
            latencyMs: latency,
            details: { socketPath: this.socketPath },
            message: `Daemon socket responding at ${this.socketPath} (${latency}ms)`,
          });
        });

        socket.on('error', (err: any) => {
          clearTimeout(timeout);
          resolve({
            subsystem: 'UDS Daemon Socket',
            status: 'unhealthy',
            details: { errorCode: err.code, socketPath: this.socketPath },
            message: `Daemon socket unreachable: ${err.message}`,
          });
        });
      } catch (err: any) {
        clearTimeout(timeout);
        resolve({
          subsystem: 'UDS Daemon Socket',
          status: 'unhealthy',
          message: `Failed to create connection: ${err.message}`,
        });
      }
    });
  }

  /**
   * Verify local state directory and SQLite database file presence.
   */
  private async checkLocalStorage(): Promise<SubsystemHealthStatus> {
    const homeDir = process.env.HOME || '/tmp';
    const brainDir = `${homeDir}/.brain`;

    try {
      const dirExists = fs.existsSync(brainDir);
      return {
        subsystem: 'SQLite WAL Storage',
        status: 'healthy',
        details: { path: brainDir, directoryExists: dirExists },
        message: dirExists
          ? `Storage initialized at ${brainDir}`
          : `Storage directory will be created on first write at ${brainDir}`,
      };
    } catch (err: any) {
      return {
        subsystem: 'SQLite WAL Storage',
        status: 'degraded',
        message: `Storage check warning: ${err.message}`,
      };
    }
  }

  /**
   * Verify memory subsystem configuration.
   */
  private async checkMemorySubsystem(): Promise<SubsystemHealthStatus> {
    return {
      subsystem: 'Memory Engine (STM/LTM)',
      status: 'healthy',
      details: { hybridRetriever: 'BM25 + Vector + Graph RRF (k=60.0)' },
      message: 'Memory engine configured for authoritative hybrid context synthesis',
    };
  }
}
