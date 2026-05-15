import { spawn } from 'child_process';
import { existsSync, createWriteStream, mkdirSync, chmodSync, unlinkSync, writeFileSync } from 'fs';
import { join } from 'path';
import { fileURLToPath } from 'url';
import { platform, arch, homedir } from 'os';
import https from 'https';

const PLUGIN_ROOT = process.env.CLAUDE_PLUGIN_ROOT || fileURLToPath(new URL('.', import.meta.url));
const IS_WIN = platform() === 'win32';
const EXT = IS_WIN ? '.exe' : '';
const BIN_NAME = `context-mode-server${EXT}`;
const CLI_BIN_NAME = `context-mode${EXT}`;
const VERSION = '1.0.10';
const HOOK_TYPES = ['posttooluse', 'pretooluse', 'precompact', 'sessionstart', 'userpromptsubmit'];

function log(...args) {
  console.error('[context-mode]', ...args);
}

function findBinary(name = BIN_NAME) {
  const candidates = [
    join(PLUGIN_ROOT, 'bin', name),
    join(PLUGIN_ROOT, '.claude-plugin', 'bin', name),
    join(PLUGIN_ROOT, 'target', 'release', name),
    join(PLUGIN_ROOT, 'target', 'debug', name),
  ];
  for (const p of candidates) {
    if (existsSync(p)) {
      return p;
    }
  }
  return null;
}

function downloadFile(url, dest) {
  return new Promise((resolve, reject) => {
    const file = createWriteStream(dest);
    const req = https.get(url, { timeout: 60000 }, (res) => {
      if (res.statusCode === 302 || res.statusCode === 301) {
        file.close();
        try { unlinkSync(dest); } catch {}
        return downloadFile(res.headers.location, dest).then(resolve).catch(reject);
      }
      if (res.statusCode !== 200) {
        file.close();
        try { unlinkSync(dest); } catch {}
        return reject(new Error(`HTTP ${res.statusCode} for ${url}`));
      }
      res.pipe(file);
      file.on('finish', () => {
        file.close();
        resolve();
      });
    });
    req.on('error', (err) => {
      try { unlinkSync(dest); } catch {}
      reject(err);
    });
    req.on('timeout', () => {
      req.destroy();
      try { unlinkSync(dest); } catch {}
      reject(new Error('Download timeout'));
    });
  });
}

async function downloadBinary(name, assetPrefix) {
  const archMap = { x64: 'x86_64', arm64: 'aarch64' };
  const platMap = { win32: 'windows', darwin: 'macos', linux: 'linux' };
  const releaseArch = archMap[arch()] || arch();
  const releasePlat = platMap[platform()] || platform();
  const assetName = `${assetPrefix}-${releasePlat}-${releaseArch}${EXT}`;
  const url = `https://github.com/ShmidtS/context-mode-rust/releases/download/v${VERSION}/${assetName}`;
  const cacheDir = join(PLUGIN_ROOT, '.claude-plugin', 'bin');
  const cachePath = join(cacheDir, name);

  try {
    mkdirSync(cacheDir, { recursive: true });
    log('Downloading', name, 'from', url);
    await downloadFile(url, cachePath);
    if (!IS_WIN) {
      chmodSync(cachePath, 0o755);
    }
    log('Binary downloaded to', cachePath);
    return cachePath;
  } catch (e) {
    log('Download failed for', name, ':', e.message);
    return null;
  }
}

function installHooks(cliPath) {
  try {
    const hooksDir = join(homedir(), '.claude', 'hooks');
    mkdirSync(hooksDir, { recursive: true });

    for (const hookType of HOOK_TYPES) {
      const hookPath = join(hooksDir, `${hookType}${IS_WIN ? '.cmd' : '.sh'}`);
      let content;
      if (IS_WIN) {
        content = `@echo off\r\n"${cliPath}" hook claude-code ${hookType} %*\r\n`;
      } else {
        content = `#!/bin/sh\n"${cliPath}" hook claude-code ${hookType} "$@"\n`;
      }
      writeFileSync(hookPath, content);
      if (!IS_WIN) {
        chmodSync(hookPath, 0o755);
      }
    }
    log('Hooks installed in', hooksDir);
  } catch (e) {
    log('Hook install warning:', e.message);
  }
}

function runCargo() {
  const cargo = IS_WIN ? 'cargo.exe' : 'cargo';
  const args = [
    'run',
    '--manifest-path', join(PLUGIN_ROOT, 'Cargo.toml'),
    '--bin', 'context-mode-server',
    '--release',
    '--quiet',
  ];
  log('Falling back to cargo run...');
  return spawn(cargo, args, {
    cwd: PLUGIN_ROOT,
    stdio: ['inherit', 'inherit', 'inherit'],
    env: process.env,
    shell: IS_WIN,
  });
}

async function main() {
  try {
    const nodeMajor = parseInt(process.version.slice(1).split('.')[0], 10);
    if (nodeMajor < 14) {
      log('Node.js 14+ required, found', process.version);
      process.exit(1);
    }

    let serverBinary = findBinary(BIN_NAME);
    if (!serverBinary) {
      serverBinary = await downloadBinary(BIN_NAME, 'context-mode-server');
    }

    let cliBinary = findBinary(CLI_BIN_NAME);
    if (!cliBinary) {
      cliBinary = await downloadBinary(CLI_BIN_NAME, 'context-mode');
    }

    if (cliBinary) {
      installHooks(cliBinary);
    }

    let child;
    if (serverBinary) {
      log('Starting', serverBinary);
      child = spawn(serverBinary, [], {
        stdio: ['inherit', 'inherit', 'inherit'],
        env: process.env,
        shell: IS_WIN,
      });
    } else {
      child = runCargo();
    }

    child.on('error', (err) => {
      log('Failed to spawn server:', err.message);
      process.exit(1);
    });

    child.on('exit', (code) => {
      process.exit(code ?? 1);
    });
  } catch (err) {
    log('Fatal error:', err.message);
    process.exit(1);
  }
}

main();
