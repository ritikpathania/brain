import { useState, useEffect } from 'react';
import { MetricsClient, DaemonMetricsData } from '../services/MetricsClient';
import { EventBus } from '../services/EventBus';

export const useMetrics = (intervalMs: number = 3000) => {
  const [metrics, setMetrics] = useState<DaemonMetricsData | null>(null);
  const [isConnected, setIsConnected] = useState(false);

  useEffect(() => {
    let active = true;

    const query = async () => {
      const data = await MetricsClient.fetchMetrics();
      if (!active) return;
      if (data) {
        setMetrics(data);
        setIsConnected(true);
        EventBus.publish({ type: 'MetricsUpdated', metrics: data });
      } else {
        setIsConnected(false);
      }
    };

    query(); // initial call
    const timer = setInterval(query, intervalMs);

    return () => {
      active = false;
      clearInterval(timer);
    };
  }, [intervalMs]);

  return { metrics, isConnected };
};
