import * as React from 'react';
import { useStdout } from './ink.js';

export function useTerminalSize(): { columns: number; rows: number } {
  const { stdout } = useStdout();
  const read = (): { columns: number; rows: number } => ({
    columns: stdout.columns ?? 80,
    rows: stdout.rows ?? 24,
  });
  const [size, setSize] = React.useState(read);
  React.useEffect(() => {
    const onResize = () => setSize(read());
    stdout.on('resize', onResize);
    return () => {
      stdout.off('resize', onResize);
    };
  }, [stdout]);
  return size;
}
