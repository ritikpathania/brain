import React, { Profiler } from 'react';
import { render } from 'ink-testing-library';
import {
  ThemeProvider,
  ThemedBox,
  ThemedText,
} from './components/design-system';
import { ThemeScenario, ResizeScenario } from './verify_tui';
import fs from 'fs';
import path from 'path';
import { execSync } from 'child_process';

// 1. Workload Work configurations
const WORKLOAD_CONFIGS = {
  logEntries: 1000,
  markdownChunks: 500,
  resizeIterations: 200,
  themeSwitches: 100,
};

// 2. Target component for workloads
interface TargetProps {
  workload: 'logs' | 'markdown' | 'resize' | 'theme';
  value: number;
}
const BenchmarkTarget: React.FC<TargetProps> = ({ workload, value }) => {
  if (workload === 'logs') {
    const logs: string[] = [];
    for (let i = 0; i < value; i++) {
      logs.push(`[TELEMETRY LOG #${i}] Processing SQLite commit to persistent LTM database store...`);
    }
    const visibleLogs = logs.slice(-15);
    return (
      <ThemedBox flexDirection="column" padding={1}>
        <ThemedText color="claude" bold>Logs Appending (Total: {value})</ThemedText>
        {visibleLogs.map((log, idx) => (
          <ThemedText key={idx} color="text">{log}</ThemedText>
        ))}
      </ThemedBox>
    );
  }

  if (workload === 'markdown') {
    let text = "";
    for (let i = 0; i < value; i++) {
      text += "Markdown text stream chunk. ";
    }
    return (
      <ThemedBox flexDirection="column" padding={1} width={80}>
        <ThemedText color="claude" bold>Markdown Response Stream</ThemedText>
        <ThemedText color="text">{text}</ThemedText>
      </ThemedBox>
    );
  }

  if (workload === 'resize') {
    const width = value % 2 === 0 ? 40 : 100;
    return (
      <ThemeProvider>
        <ResizeScenario width={width} />
      </ThemeProvider>
    );
  }

  if (workload === 'theme') {
    const themesList = ['dark', 'light', 'dark-daltonized', 'light-daltonized', 'dark-ansi', 'light-ansi'];
    const activeTheme = themesList[value % themesList.length];
    return (
      <ThemeProvider key={activeTheme} defaultTheme={activeTheme as any}>
        <ThemeScenario />
      </ThemeProvider>
    );
  }

  return null;
};

// 3. Main runner
async function runRenderBenchmarks() {
  console.log('--- STARTING DETERMINISTIC RENDER BENCHMARKS ---');

  const scenarioMetrics: Record<string, any> = {};
  let durList: number[] = [];

  const onRender = (
    id: string,
    phase: 'mount' | 'update',
    actualDuration: number
  ) => {
    durList.push(actualDuration);
  };

  const executeWorkload = (
    name: 'logs' | 'markdown' | 'resize' | 'theme',
    iterations: number
  ) => {
    console.log(`Profiling workload: '${name}' (${iterations} frames)...`);
    durList = [];

    const { rerender, unmount } = render(
      <Profiler id={name} onRender={onRender}>
        <BenchmarkTarget workload={name} value={0} />
      </Profiler>
    );

    const startWall = performance.now();
    for (let i = 1; i <= iterations; i++) {
      rerender(
        <Profiler id={name} onRender={onRender}>
          <BenchmarkTarget workload={name} value={i} />
        </Profiler>
      );
    }
    const totalWallMs = performance.now() - startWall;

    unmount();

    const framesCount = durList.length;
    const totalRenderMs = durList.reduce((sum, d) => sum + d, 0);
    const averageCommitMs = framesCount > 0 ? (totalRenderMs / framesCount) : 0;
    const maxCommitMs = durList.length > 0 ? Math.max(...durList) : 0;

    const sorted = [...durList].sort((a, b) => a - b);
    let medianCommitMs = 0;
    if (sorted.length > 0) {
      if (sorted.length % 2 !== 0) {
        medianCommitMs = sorted[Math.floor(sorted.length / 2)];
      } else {
        medianCommitMs = (sorted[sorted.length / 2 - 1] + sorted[sorted.length / 2]) / 2;
      }
    }

    const p95CommitMs = sorted.length > 0
      ? sorted[Math.min(sorted.length - 1, Math.floor(sorted.length * 0.95))]
      : 0;

    let stdDevCommitMs = 0;
    if (framesCount > 1) {
      const sqDiffs = durList.map(d => Math.pow(d - averageCommitMs, 2));
      const sumSqDiffs = sqDiffs.reduce((sum, d) => sum + d, 0);
      stdDevCommitMs = Math.sqrt(sumSqDiffs / framesCount);
    }

    scenarioMetrics[name] = {
      totalWallMs: parseFloat(totalWallMs.toFixed(3)),
      totalRenderMs: parseFloat(totalRenderMs.toFixed(3)),
      averageCommitMs: parseFloat(averageCommitMs.toFixed(3)),
      medianCommitMs: parseFloat(medianCommitMs.toFixed(3)),
      p95CommitMs: parseFloat(p95CommitMs.toFixed(3)),
      stdDevCommitMs: parseFloat(stdDevCommitMs.toFixed(3)),
      maxCommitMs: parseFloat(maxCommitMs.toFixed(3)),
      frames: framesCount,
      sampleCount: framesCount,
    };

    console.log(`  └─ Frames: ${framesCount}`);
    console.log(`  └─ Total Profiler Render: ${totalRenderMs.toFixed(3)} ms`);
    console.log(`  └─ Avg Commit/Frame: ${averageCommitMs.toFixed(3)} ms`);
    console.log(`  └─ Median Commit: ${medianCommitMs.toFixed(3)} ms`);
    console.log(`  └─ p95 Commit: ${p95CommitMs.toFixed(3)} ms`);
    console.log(`  └─ StdDev Commit: ${stdDevCommitMs.toFixed(3)} ms`);
    console.log(`  └─ Max Commit: ${maxCommitMs.toFixed(3)} ms`);
    console.log(`  └─ Wall Time: ${totalWallMs.toFixed(3)} ms\n`);
  };

  // Run the workloads
  executeWorkload('logs', WORKLOAD_CONFIGS.logEntries);
  executeWorkload('markdown', WORKLOAD_CONFIGS.markdownChunks);
  executeWorkload('resize', WORKLOAD_CONFIGS.resizeIterations);
  executeWorkload('theme', WORKLOAD_CONFIGS.themeSwitches);

  // 4. Gather system/git metadata
  let gitCommit = process.env.GITHUB_SHA ||
                  process.env.GIT_COMMIT ||
                  process.env.CI_COMMIT_SHA ||
                  process.env.COMMIT_SHA;
  if (!gitCommit) {
    try {
      gitCommit = execSync('git rev-parse HEAD 2>/dev/null').toString().trim();
    } catch (err) {}
  }

  if (!gitCommit || gitCommit === 'unknown') {
    const isCI = process.env.CI === 'true' || process.env.GITHUB_ACTIONS === 'true';
    if (isCI) {
      console.error('\n[Error] Git commit metadata is mandatory in CI environments. Set GITHUB_SHA / GIT_COMMIT / CI_COMMIT_SHA or ensure a git repo is initialized.');
      process.exit(1);
    } else {
      console.warn('\n[Warning] Git commit metadata not found. Benchmark trend reports require a Git commit.');
      console.warn('  Ensure a git repository is initialized or set the GIT_COMMIT environment variable.');
      gitCommit = 'unknown';
    }
  }

  const reportData = {
    schemaVersion: 1,
    timestamp: new Date().toISOString(),
    gitCommit,
    bunVersion: Bun.version,
    platform: `${process.platform}-${process.arch}`,
    config: WORKLOAD_CONFIGS,
    scenarios: scenarioMetrics,
  };

  const reportPath = path.resolve(__dirname, '../benchmark_render_report.json');
  fs.writeFileSync(reportPath, JSON.stringify(reportData, null, 2));
  console.log(`JSON report saved successfully to: ${reportPath}`);

  // Save historical timestamped report
  try {
    const benchmarksDir = path.resolve(__dirname, '../benchmarks');
    if (!fs.existsSync(benchmarksDir)) {
      fs.mkdirSync(benchmarksDir, { recursive: true });
    }
    const timestampStr = reportData.timestamp.replace(/:/g, '-');
    const histPath = path.join(benchmarksDir, `render_report_${timestampStr}.json`);
    fs.writeFileSync(histPath, JSON.stringify(reportData, null, 2));
    console.log(`Historical report saved to: ${histPath}`);
  } catch (err: any) {
    console.warn('Could not save historical report:', err.message);
  }

  // 5. Baseline Comparison (CI protection)
  const baselinePath = path.resolve(__dirname, '../benchmark_render_baseline.json');
  if (fs.existsSync(baselinePath)) {
    console.log('\n--- COMPARING AGAINST BASELINE ---');
    try {
      const baseline = JSON.parse(fs.readFileSync(baselinePath, 'utf8'));
      let regressionDetected = false;

      for (const scenario in WORKLOAD_CONFIGS) {
        const key = scenario.replace('Entries', '').replace('Chunks', '').replace('Iterations', '').replace('Switches', '').toLowerCase();
        const baseVal = baseline.scenarios[key]?.averageCommitMs;
        const currentVal = reportData.scenarios[key]?.averageCommitMs;

        if (baseVal !== undefined && currentVal !== undefined) {
          const increasePercent = ((currentVal - baseVal) / baseVal) * 100;
          console.log(`Scenario '${key}': Baseline: ${baseVal.toFixed(3)}ms | Current: ${currentVal.toFixed(3)}ms (${increasePercent >= 0 ? '+' : ''}${increasePercent.toFixed(1)}%)`);
          
          // Flag if regression exceeds 30%
          if (increasePercent > 30.0) {
            console.log(`  [Warning]: Performance regression of ${increasePercent.toFixed(1)}% detected in '${key}'!`);
            regressionDetected = true;
          }
        }
      }

      if (regressionDetected && process.env.BRAIN_FAIL_ON_PERF_REGRESSION === 'true') {
        console.error('\n[Error]: Performance regression detected. Failing CI build.');
        process.exit(1);
      }
    } catch (err) {
      console.warn('Could not parse baseline file:', err);
    }
  } else {
    console.log('\nNo baseline file found. Save this report to benchmark_render_baseline.json to track regressions.');
  }

  console.log('\n--- PROFILING COMPLETED ---');
}

runRenderBenchmarks();
