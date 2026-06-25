import net from 'net';
import path from 'path';

const homeDir = process.env.HOME || '/tmp';
const SOCKET_PATH = process.env.BRAIN_SOCKET_PATH || path.join(homeDir, '.brain', 'daemon.sock');

async function runTest() {
  console.log(`[LTM Test] Connecting to UDS socket at: ${SOCKET_PATH}`);
  const client = net.createConnection({ path: SOCKET_PATH });

  client.on('connect', async () => {
    console.log('[LTM Test] Connected to daemon socket!');
    
    // Ingest 2 entries
    console.log("[LTM Test] Ingesting log 1...");
    client.write(JSON.stringify({ action: 'ingest', payload: 'sqlite database configuration setup' }) + '\n');
    await sleep(500);

    console.log("[LTM Test] Ingesting log 2...");
    client.write(JSON.stringify({ action: 'ingest', payload: 'storing API keys in environment variables' }) + '\n');
    await sleep(500);

    console.log("[LTM Test] Waiting 35 seconds for background consolidation to flush STM into SQLite LTM...");
    
    // Countdown timer for user visibility
    for (let i = 35; i > 0; i -= 5) {
      console.log(`  ... ${i}s remaining`);
      await sleep(5000);
    }

    console.log("[LTM Test] Performing query for 'db config' (should miss STM and fetch from LTM)...");
    client.write(JSON.stringify({ action: 'query', payload: 'db config' }) + '\n');
    await sleep(1000);

    console.log("[LTM Test] Performing query for 'api key' (should miss STM and fetch from LTM)...");
    client.write(JSON.stringify({ action: 'query', payload: 'api key' }) + '\n');
    await sleep(1000);

    console.log('[LTM Test] Closing connection.');
    client.end();
    process.exit(0);
  });

  client.on('data', (data) => {
    const lines = data.toString().split('\n').filter(Boolean);
    for (const line of lines) {
      try {
        const response = JSON.parse(line);
        console.log(`\n[LTM Test Response]:\n----------------\n${response.message}\n----------------`);
      } catch (e) {
        console.error("[LTM Test Error] Parse error:", e);
      }
    }
  });

  client.on('error', (err) => {
    console.error(`[LTM Test Error] Connection failed: ${err.message}`);
    process.exit(1);
  });
}

function sleep(ms: number) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

runTest();
