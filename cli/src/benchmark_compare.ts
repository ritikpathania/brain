import fs from 'fs';
import path from 'path';

function getPercentDiff(current: number, baseline: number): number {
  if (baseline === 0) return 0;
  return ((current - baseline) / baseline) * 100;
}

function formatPercent(diff: number): string {
  const sign = diff >= 0 ? '+' : '';
  return `${sign}${diff.toFixed(1)}%`;
}

function classifyChange(diff: number): 'Stable' | 'Noticeable' | 'Significant' {
  const abs = Math.abs(diff);
  if (abs < 2.0) return 'Stable';
  if (abs <= 10.0) return 'Noticeable';
  return 'Significant';
}

function getIndicator(classification: 'Stable' | 'Noticeable' | 'Significant', diff: number): string {
  if (classification === 'Stable') return '[Stable]';
  if (diff > 0) {
    return classification === 'Significant' ? '[Significant Regression ⚠️]' : '[Noticeable Regression 📈]';
  } else {
    return classification === 'Significant' ? '[Significant Improvement 🚀]' : '[Noticeable Improvement 📉]';
  }
}

function runComparison() {
  const args = process.argv.slice(2);

  // Check for --last or -l argument
  const lastIdx = args.findIndex(arg => arg === '--last' || arg === '-l');
  if (lastIdx !== -1) {
    const limit = parseInt(args[lastIdx + 1], 10) || 5;
    showHistoricalTrends(limit);
    return;
  }

  const reportPath = args[0] 
    ? path.resolve(args[0]) 
    : path.resolve(__dirname, '../benchmark_render_report.json');
  
  const baselinePath = args[1] 
    ? path.resolve(args[1]) 
    : path.resolve(__dirname, '../benchmark_render_baseline.json');

  if (!fs.existsSync(reportPath)) {
    console.error(`Error: Report file not found at ${reportPath}`);
    process.exit(1);
  }

  if (!fs.existsSync(baselinePath)) {
    console.error(`Error: Baseline file not found at ${baselinePath}`);
    console.log('To set a baseline, copy your report:');
    console.log(`  cp ${path.relative(process.cwd(), reportPath)} ${path.relative(process.cwd(), baselinePath)}`);
    process.exit(1);
  }

  try {
    const report = JSON.parse(fs.readFileSync(reportPath, 'utf8'));
    const baseline = JSON.parse(fs.readFileSync(baselinePath, 'utf8'));

    console.log('=== BENCHMARK COMPARISON REPORT ===');
    console.log(`Current:  ${report.timestamp || 'N/A'} (Commit: ${(report.gitCommit || 'N/A').slice(0, 7)})`);
    console.log(`Baseline: ${baseline.timestamp || 'N/A'} (Commit: ${(baseline.gitCommit || 'N/A').slice(0, 7)})`);
    console.log('===================================');

    const currentScenarios = report.scenarios || {};
    const baselineScenarios = baseline.scenarios || {};

    let stableCount = 0;
    let improvedCount = 0;
    let regressedCount = 0;

    for (const name of Object.keys(currentScenarios)) {
      const cur = currentScenarios[name];
      const base = baselineScenarios[name];

      if (!base) {
        console.log(`\n${name.toUpperCase()} (No baseline data)`);
        continue;
      }

      console.log(`\n${name.charAt(0).toUpperCase() + name.slice(1)}`);
      
      const avgDiff = getPercentDiff(cur.averageCommitMs, base.averageCommitMs);
      const maxDiff = getPercentDiff(cur.maxCommitMs, base.maxCommitMs);
      
      const avgClass = classifyChange(avgDiff);
      const maxClass = classifyChange(maxDiff);

      // Count trends based on average commit time
      if (avgClass === 'Stable') {
        stableCount++;
      } else if (avgDiff < 0) {
        improvedCount++;
      } else {
        regressedCount++;
      }

      console.log(`  Avg: ${cur.averageCommitMs.toFixed(3)}ms vs ${base.averageCommitMs.toFixed(3)}ms (${formatPercent(avgDiff)}) ${getIndicator(avgClass, avgDiff)}`);
      
      if (cur.medianCommitMs !== undefined && base.medianCommitMs !== undefined) {
        const medDiff = getPercentDiff(cur.medianCommitMs, base.medianCommitMs);
        const medClass = classifyChange(medDiff);
        console.log(`  Med: ${cur.medianCommitMs.toFixed(3)}ms vs ${base.medianCommitMs.toFixed(3)}ms (${formatPercent(medDiff)}) ${getIndicator(medClass, medDiff)}`);
      }
      if (cur.p95CommitMs !== undefined && base.p95CommitMs !== undefined) {
        const p95Diff = getPercentDiff(cur.p95CommitMs, base.p95CommitMs);
        const p95Class = classifyChange(p95Diff);
        console.log(`  p95: ${cur.p95CommitMs.toFixed(3)}ms vs ${base.p95CommitMs.toFixed(3)}ms (${formatPercent(p95Diff)}) ${getIndicator(p95Class, p95Diff)}`);
      }
      if (cur.stdDevCommitMs !== undefined && base.stdDevCommitMs !== undefined) {
        console.log(`  SD : ${cur.stdDevCommitMs.toFixed(3)}ms vs ${base.stdDevCommitMs.toFixed(3)}ms`);
      }
      
      console.log(`  Max: ${cur.maxCommitMs.toFixed(3)}ms vs ${base.maxCommitMs.toFixed(3)}ms (${formatPercent(maxDiff)}) ${getIndicator(maxClass, maxDiff)}`);
      console.log(`  Samples: ${cur.frames} (Current) vs ${base.frames} (Baseline)`);
    }

    console.log('\n===================================');
    console.log('Overall Summary:');
    console.log(`  Stable:    ${stableCount} workload(s)`);
    console.log(`  Improved:  ${improvedCount} workload(s)`);
    console.log(`  Regressed: ${regressedCount} workload(s)`);
    console.log('===================================');
  } catch (err: any) {
    console.error('Error parsing or reading JSON files:', err.message);
    process.exit(1);
  }
}

function showHistoricalTrends(limit: number) {
  const benchmarksDir = path.resolve(__dirname, '../benchmarks');
  if (!fs.existsSync(benchmarksDir)) {
    console.log('No historical reports directory found. Run rendering benchmarks first.');
    return;
  }

  const files = fs.readdirSync(benchmarksDir)
    .filter(f => f.startsWith('render_report_') && f.endsWith('.json'))
    .map(f => {
      const filePath = path.join(benchmarksDir, f);
      const stat = fs.statSync(filePath);
      return { file: filePath, time: stat.mtimeMs };
    });

  if (files.length === 0) {
    console.log('No historical reports found in benchmarks directory.');
    return;
  }

  // Sort files chronologically
  files.sort((a, b) => a.time - b.time);

  // Take the last N reports
  const selectedFiles = files.slice(-limit);

  console.log(`\n=== HISTORICAL PERFORMANCE TRENDS (Last ${selectedFiles.length} runs) ===`);

  try {
    const runs = selectedFiles.map(f => JSON.parse(fs.readFileSync(f.file, 'utf8')));
    const scenarios = Object.keys(runs[0].scenarios);

    // Track overall status counts for the latest run
    let latestStableCount = 0;
    let latestImprovedCount = 0;
    let latestRegressedCount = 0;

    for (const name of scenarios) {
      console.log(`\nWorkload: ${name.charAt(0).toUpperCase() + name.slice(1)}`);
      
      const prevVals: number[] = [];
      
      for (let i = 0; i < runs.length; i++) {
        const run = runs[i];
        const val = run.scenarios[name]?.averageCommitMs ?? 0;
        const commit = (run.gitCommit || 'N/A').slice(0, 7);
        const time = new Date(run.timestamp).toLocaleTimeString();
        const date = new Date(run.timestamp).toLocaleDateString();

        if (prevVals.length === 0) {
          console.log(`  [${date} ${time}] Commit: ${commit} | Avg: ${val.toFixed(3)}ms (Baseline)`);
        } else {
          // Calculate running average of all previous runs in the window to smooth out variance
          const runningAvg = prevVals.reduce((sum, v) => sum + v, 0) / prevVals.length;
          const diff = getPercentDiff(val, runningAvg);
          const cls = classifyChange(diff);
          
          console.log(`  [${date} ${time}] Commit: ${commit} | Avg: ${val.toFixed(3)}ms (${formatPercent(diff)} vs running avg ${runningAvg.toFixed(3)}ms) ${getIndicator(cls, diff)}`);
          
          // If this is the latest run in the window, register overall status
          if (i === runs.length - 1) {
            if (cls === 'Stable') {
              latestStableCount++;
            } else if (diff < 0) {
              latestImprovedCount++;
            } else {
              latestRegressedCount++;
            }
          }
        }
        prevVals.push(val);
      }
    }

    if (runs.length > 1) {
      console.log('\n======================================================');
      console.log('Overall Trend Summary (Latest Run vs Running Avg):');
      console.log(`  Stable:    ${latestStableCount} workload(s)`);
      console.log(`  Improved:  ${latestImprovedCount} workload(s)`);
      console.log(`  Regressed: ${latestRegressedCount} workload(s)`);
    }
    console.log('======================================================');
  } catch (err: any) {
    console.error('Error compiling historical trends:', err.message);
  }
}

runComparison();
