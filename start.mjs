import { spawn } from 'node:child_process';
import { existsSync } from 'node:fs';
import { join, resolve } from 'node:path';

const PLUGIN_ROOT = process.env.CLAUDE_PLUGIN_ROOT || resolve(new URL('.', import.meta.url).pathname);
const isWindows = process.platform === 'win32';
const EXT = isWindows ? '.exe' : '';

function findBinary() {
  const candidates = [
    join(PLUGIN_ROOT, 'bin', `context-mode-server${EXT}`),
    join(PLUGIN_ROOT, '.claude-plugin', 'bin', `context-mode-server${EXT}`),
    join(PLUGIN_ROOT, 'target', 'release', `context-mode-server${EXT}`),
    join(PLUGIN_ROOT, 'target', 'debug', `context-mode-server${EXT}`),
  ];

  for (const path of candidates) {
    if (existsSync(path)) {
      return path;
    }
  }

  return null;
}

function runCargo() {
  const cargo = isWindows ? 'cargo.exe' : 'cargo';
  const args = [
    'run',
    '--manifest-path', join(PLUGIN_ROOT, 'Cargo.toml'),
    '--bin', 'context-mode-server',
    '--release',
    '--quiet',
  ];

  return spawn(cargo, args, {
    cwd: PLUGIN_ROOT,
    stdio: ['inherit', 'inherit', 'inherit'],
    env: process.env,
    shell: isWindows,
  });
}

function main() {
  const binary = findBinary();

  let child;
  if (binary) {
    child = spawn(binary, [], {
      stdio: ['inherit', 'inherit', 'inherit'],
      env: process.env,
      shell: isWindows,
    });
  } else {
    console.error('[context-mode-rust] Binary not found, falling back to cargo run...');
    child = runCargo();
  }

  child.on('error', (err) => {
    console.error('[context-mode-rust] Failed to spawn server:', err.message);
    process.exit(1);
  });

  child.on('exit', (code) => {
    process.exit(code ?? 1);
  });
}

main();
