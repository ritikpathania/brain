import { spawn, execSync } from 'child_process';
import net from 'net';
import fs from 'fs';
import path from 'path';

const homeDir = process.env.HOME || '/tmp';
const SOCKET_PATH = process.env.BRAIN_SOCKET_PATH || path.join(homeDir, '.brain', 'daemon.sock');
const HEALTH_URL = 'http://127.0.0.1:8080';
const DAEMON_BIN = path.resolve(__dirname, '../../daemon/target/debug/brain');
const PYTHON_PATH = path.resolve(__dirname, '../../daemon/.venv/bin/python');
const PYTHONPATH = `${path.resolve(__dirname, '../../daemon')}:${path.resolve(__dirname, '../../daemon/.venv/lib/python3.12/site-packages')}`;

function sleep(ms: number) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

async function runE2ETests() {
  console.log('--- STARTING E2E TEST ---');

  // 1. Cleanup any existing socket or daemon process
  try {
    if (fs.existsSync(SOCKET_PATH)) {
      console.log(`Cleaning up old socket file: ${SOCKET_PATH}`);
      fs.unlinkSync(SOCKET_PATH);
    }
  } catch (err) {
    // Ignore
  }

  // Kill any existing daemon processes
  try {
    console.log('Stopping any running daemon processes...');
    if (fs.existsSync(DAEMON_BIN)) {
      execSync(`${DAEMON_BIN} daemon stop || true`);
    } else {
      execSync('pkill -f "target/debug/brain" || true');
    }
    await sleep(1000);
  } catch (err) {
    // Ignore
  }

  // 2. Start the daemon process
  console.log(`Spawning daemon binary: ${DAEMON_BIN}`);
  const env = {
    ...process.env,
    PYO3_PYTHON: PYTHON_PATH,
    PYTHONPATH: PYTHONPATH,
    LOG_FORMAT: 'json',
    LOG_LEVEL: 'debug',
  };

  const daemon = spawn(DAEMON_BIN, ['daemon', 'run'], {
    env,
    cwd: path.resolve(__dirname, '../../daemon'),
  });

  let daemonLogs = '';
  daemon.stdout.on('data', (data) => {
    daemonLogs += data.toString();
  });
  daemon.stderr.on('data', (data) => {
    daemonLogs += data.toString();
  });

  daemon.on('close', (code) => {
    console.log(`Daemon process exited with code ${code}`);
  });

  // 3. Poll /health for liveness
  console.log('Polling /health endpoint...');
  let healthy = false;
  for (let i = 0; i < 50; i++) {
    try {
      const res = await fetch(`${HEALTH_URL}/health`);
      if (res.ok) {
        const body = (await res.json()) as any;
        if (body.status === 'ok') {
          healthy = true;
          console.log('/health is ok!');
          break;
        }
      }
    } catch (err) {
      // Ignore fetch errors while waiting
    }
    await sleep(100);
  }

  if (!healthy) {
    console.error('Daemon failed to become healthy within 5 seconds!');
    console.error('Daemon Logs:');
    console.error(daemonLogs);
    daemon.kill('SIGKILL');
    process.exit(1);
  }

  // Verify /ready
  try {
    const res = await fetch(`${HEALTH_URL}/ready`);
    const body = (await res.json()) as any;
    console.log(`/ready response: ${JSON.stringify(body)}`);
    if (body.status !== 'ready') {
      throw new Error('/ready returned incorrect status');
    }
  } catch (err: any) {
    console.error(`Readiness check failed: ${err.message}`);
    daemon.kill('SIGKILL');
    process.exit(1);
  }

  // Verify /metrics
  try {
    const res = await fetch(`${HEALTH_URL}/metrics`);
    const body = await res.text();
    console.log('/metrics matches expected keys:', body.includes('cache_hits'));
  } catch (err: any) {
    console.error(`Metrics check failed: ${err.message}`);
    daemon.kill('SIGKILL');
    process.exit(1);
  }

  // 4. Ingest data over UDS
  console.log(`Connecting to UDS socket at: ${SOCKET_PATH}`);
  const client = net.createConnection({ path: SOCKET_PATH });

  let ingestResponse: any = null;
  let queryResponse: any = null;

  client.on('connect', () => {
    console.log('Connected to daemon UDS socket!');
    
    // Send ingest payload
    const ingestMsg = { action: 'ingest', payload: 'sqlite database configuration setup' };
    console.log(`Sending ingest command: ${JSON.stringify(ingestMsg)}`);
    client.write(JSON.stringify(ingestMsg) + '\n');
  });

  client.on('data', async (data) => {
    const lines = data.toString().split('\n').filter(Boolean);
    for (const line of lines) {
      const response = JSON.parse(line);
      console.log(`UDS Response: ${JSON.stringify(response)}`);
      if (!ingestResponse) {
        ingestResponse = response;
        
        // Now send query command
        const queryMsg = { action: 'query', payload: 'db config' };
        console.log(`Sending query command: ${JSON.stringify(queryMsg)}`);
        client.write(JSON.stringify(queryMsg) + '\n');
      } else if (!queryResponse) {
        queryResponse = response;
        client.end();
      }
    }
  });

  client.on('error', (err) => {
    console.error(`Socket error: ${err.message}`);
  });

  // Wait for socket interactions to complete
  for (let i = 0; i < 50; i++) {
    if (queryResponse) break;
    await sleep(100);
  }

  if (!ingestResponse || !queryResponse) {
    console.error('Failed to complete UDS ingest/query exchange');
    daemon.kill('SIGKILL');
    process.exit(1);
  }

  console.log('UDS ingest/query exchange succeeded!');

  // 5. Verify semantic extraction works (calling PyO3/Python extractor)
  // To verify this, we need to wait for background consolidation.
  // The consolidation check runs every 30 seconds. Let's wait 35 seconds.
  console.log('Waiting 35 seconds for background consolidation & DuckDB sync...');
  for (let i = 35; i > 0; i -= 5) {
    console.log(`  ... ${i}s remaining`);
    await sleep(5000);
  }

  // 6. Assert incremental sync to DuckDB
  console.log('Querying DuckDB analytics summary...');
  try {
    const res = await fetch(`${HEALTH_URL}/analytics/summary`);
    const summary = (await res.json()) as any;
    console.log('DuckDB Summary:', JSON.stringify(summary, null, 2));

    // Assert that total_nodes >= 1 (or 2 depending on extraction) and total_edges is present
    if (summary.total_nodes === 0) {
      throw new Error('DuckDB total_nodes is 0! Incremental sync failed or extractor failed.');
    }
    console.log('E2E ASSERTION PASSED: DuckDB contains synchronized nodes!');
  } catch (err: any) {
    console.error(`DuckDB sync verification failed: ${err.message}`);
    console.error('Daemon Logs:');
    console.error(daemonLogs);
    daemon.kill('SIGKILL');
    process.exit(1);
  }

  // 7. Graceful shutdown
  console.log('Stopping daemon process...');
  daemon.kill('SIGTERM');
  
  // Wait for exit
  await sleep(1000);
  console.log('--- E2E TEST COMPLETED SUCCESSFULLY ---');
  process.exit(0);
}

runE2ETests();
