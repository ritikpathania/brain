import net from 'net';
import path from 'path';

const homeDir = process.env.HOME || '/tmp';
const SOCKET_PATH = process.env.BRAIN_SOCKET_PATH || path.join(homeDir, '.brain', 'daemon.sock');

function runTest() {
  console.log(`[Test] Connecting to UDS socket at: ${SOCKET_PATH}`);
  const client = net.createConnection({ path: SOCKET_PATH });

  let step = 0;
  let receivedCount = 0;

  const payloads = [
    { action: 'ingest', payload: 'sqlite database configuration setup' },
    { action: 'ingest', payload: 'storing API keys in environment variables' },
    { action: 'query', payload: 'db config' },
    { action: 'query', payload: 'api key' }
  ];

  client.on('connect', () => {
    console.log('[Test] Connected to daemon socket!');
    sendNext();
  });

  function sendNext() {
    if (step < payloads.length) {
      const msg = payloads[step];
      console.log(`[Test] Sending Action: '${msg.action}', Payload: '${msg.payload}'`);
      client.write(JSON.stringify(msg) + '\n');
      step++;
    }
  }

  let buffer = '';
  client.on('data', (data) => {
    buffer += data.toString();
    let newlineIdx;
    while ((newlineIdx = buffer.indexOf('\n')) !== -1) {
      const line = buffer.slice(0, newlineIdx).trim();
      buffer = buffer.slice(newlineIdx + 1);
      if (line) {
        console.log(`[Test] Response received:\n----------------\n${JSON.parse(line).message}\n----------------`);
        receivedCount++;
        if (receivedCount === payloads.length) {
          console.log('[Test] All tests completed successfully! Closing socket.');
          client.end();
          process.exit(0);
        } else {
          sendNext();
        }
      }
    }
  });

  client.on('error', (err) => {
    console.error(`[Test Error] Socket connection failed: ${err.message}`);
    process.exit(1);
  });

  client.on('end', () => {
    console.log('[Test] Connection ended by daemon.');
  });
}

runTest();
