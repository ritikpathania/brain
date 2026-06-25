import net from 'net';
import path from 'path';

const homeDir = process.env.HOME || '/tmp';
const SOCKET_PATH = process.env.BRAIN_SOCKET_PATH || path.join(homeDir, '.brain', 'daemon.sock');
const HEALTH_PORT = 8080;

async function queryAnalytics(endpoint: string): Promise<any> {
  const url = `http://127.0.0.1:${HEALTH_PORT}${endpoint}`;
  try {
    const res = await fetch(url);
    if (!res.ok) {
      throw new Error(`HTTP error! status: ${res.status}`);
    }
    return await res.json();
  } catch (e: any) {
    console.error(`[Analytics HTTP Error] Failed to fetch ${endpoint}: ${e.message}`);
    return null;
  }
}

async function runTest() {
  console.log(`[Analytics Test] Connecting to UDS socket at: ${SOCKET_PATH}`);
  const client = net.createConnection({ path: SOCKET_PATH });

  client.on('connect', async () => {
    console.log('[Analytics Test] Connected to daemon socket!');
    
    // Ingest 3 entries
    console.log("[Analytics Test] Ingesting log 1...");
    client.write(JSON.stringify({ action: 'ingest', payload: 'sqlite database configuration setup' }) + '\n');
    await sleep(500);

    console.log("[Analytics Test] Ingesting log 2...");
    client.write(JSON.stringify({ action: 'ingest', payload: 'storing API keys in environment variables' }) + '\n');
    await sleep(500);

    console.log("[Analytics Test] Ingesting log 3...");
    client.write(JSON.stringify({ action: 'ingest', payload: 'deploying node.js server to AWS production environment' }) + '\n');
    await sleep(500);

    // Queries to record query logs
    console.log("[Analytics Test] Querying for 'db config' (should hit STM)...");
    client.write(JSON.stringify({ action: 'query', payload: 'db config' }) + '\n');
    await sleep(500);

    console.log("[Analytics Test] Querying for 'api key' (should hit STM)...");
    client.write(JSON.stringify({ action: 'query', payload: 'api key' }) + '\n');
    await sleep(500);

    console.log("[Analytics Test] Querying for 'nonexistent keyword' (should miss)...");
    client.write(JSON.stringify({ action: 'query', payload: 'nonexistent keyword' }) + '\n');
    await sleep(500);

    console.log("[Analytics Test] Waiting 35 seconds for background consolidation and SQLite-to-DuckDB sync...");
    for (let i = 35; i > 0; i -= 5) {
      console.log(`  ... ${i}s remaining`);
      await sleep(5000);
    }

    console.log("\n==================================================");
    console.log("FETCHING DUCKDB ANALYTICAL INSIGHTS");
    console.log("==================================================");

    const summary = await queryAnalytics('/analytics/summary');
    console.log("\n[1. Summary Statistics]:");
    console.log(JSON.stringify(summary, null, 2));

    const insights = await queryAnalytics('/analytics/insights');
    console.log("\n[2. Graph Insights (Centrality & Node Types)]:");
    console.log(JSON.stringify(insights, null, 2));

    const similarity = await queryAnalytics('/analytics/similarity');
    console.log("\n[3. Node Similarity Report]:");
    console.log(JSON.stringify(similarity, null, 2));

    const slowQueries = await queryAnalytics('/analytics/slow-queries');
    console.log("\n[4. Latency Benchmarks & Slow Queries]:");
    console.log(JSON.stringify(slowQueries, null, 2));

    console.log('\n[Analytics Test] Closing connection.');
    client.end();
    process.exit(0);
  });

  client.on('data', (data) => {
    const lines = data.toString().split('\n').filter(Boolean);
    for (const line of lines) {
      try {
        const response = JSON.parse(line);
        console.log(`\n[Analytics Test Response]: ${response.status} -> ${response.message.split('\n')[0]}`);
      } catch (e) {
        // Suppress parsing logs of unstructured print statements
      }
    }
  });

  client.on('error', (err) => {
    console.error(`[Analytics Test Error] Connection failed: ${err.message}`);
    process.exit(1);
  });
}

function sleep(ms: number) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

runTest();
