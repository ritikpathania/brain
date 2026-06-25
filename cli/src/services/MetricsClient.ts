export interface DaemonMetricsData {
  cache_hit_rate: number;
  cache_hits: number;
  cache_misses: number;
  total_queries: number;
  total_ingests: number;
  active_workers: number;
  queue_depth: number;
  avg_query_latency_us: number;
  avg_ingest_latency_us: number;
  avg_extraction_latency_us: number;
  avg_sqlite_latency_us: number;
  avg_ipc_latency_us: number;
}

export class MetricsClient {
  private static endpoint = 'http://127.0.0.1:8080/metrics/json';

  public static async fetchMetrics(): Promise<DaemonMetricsData | null> {
    try {
      const res = await fetch(this.endpoint, { signal: AbortSignal.timeout(1500) });
      if (!res.ok) return null;
      return await res.json();
    } catch (e) {
      return null;
    }
  }
}
