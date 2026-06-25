import { useState, useEffect } from 'react';
import { EventBus } from '../services/EventBus';

export const useLogStore = () => {
  const [logs, setLogs] = useState<string[]>([]);

  useEffect(() => {
    const unsubscribe = EventBus.subscribe((event) => {
      if (event.type === 'ToastAdded') {
        setLogs((prev) => [...prev, `[System Toast] ${event.message}`]);
      }
    });
    return unsubscribe;
  }, []);

  const addLog = (log: string) => {
    setLogs((prev) => [...prev, log]);
  };

  return { logs, setLogs, addLog };
};
