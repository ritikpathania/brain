#!/usr/bin/env node

const { spawn } = require('child_process');
const path = require('path');
const fs = require('fs');

// Resolve path to the compiled Rust binary
const binaryPath = path.join(__dirname, 'brain');

if (!fs.existsSync(binaryPath)) {
  console.error(`Error: 'brain' native binary not found at: ${binaryPath}`);
  console.error('Please run "bun run build" or compile the Rust daemon first.');
  process.exit(1);
}

// Forward all command-line arguments directly to the Rust binary
const args = process.argv.slice(2);
const child = spawn(binaryPath, args, {
  stdio: 'inherit', // Inherit stdin/stdout/stderr for interactive TUI support
  env: process.env
});

child.on('close', (code) => {
  process.exit(code ?? 0);
});

child.on('error', (err) => {
  console.error(`Error launching brain binary: ${err.message}`);
  process.exit(1);
});
