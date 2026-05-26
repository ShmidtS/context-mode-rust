import { spawn } from 'child_process';
import { existsSync, createWriteStream, mkdirSync, chmodSync, unlinkSync, writeFileSync, readFileSync, statSync } from 'fs';
import { join, dirname } from 'path';
import { fileURLToPath } from 'url';
import { platform, arch, homedir } from 'os';
import https from 'https';

const PLUGIN_ROOT = process.env.CLAUDE_PLUGIN_ROOT || fileURLToPath(new URL('.', import.meta.url));
const IS_WIN = platform() === 'win32';
const EXT = IS_WIN ? '.exe' : '';
const BIN_NAME = `context-mode-server${EXT}`;
const CLI_BIN_NAME = `context-mode${EXT}`;
const INSIGHT_BIN_NAME = `context-mode-insight${EXT}`;
const VERSION = '1.3.7';
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

function removeUnixShimsOnWindows(dir) {
  if (!IS_WIN) return;
  for (const name of ['context-mode', 'context-mode-server', 'context-mode-insight']) {
    const shim = join(dir, name);
    try {
      if (existsSync(shim)) {
        const s = statSync(shim);
        if (s.isFile() && s.size < 4096) {
          unlinkSync(shim);
          log('Removed Unix shim on Windows:', shim);
        }
      }
    } catch (e) {
      // ignore
    }
  }
}

function installHooks(cliPath) {
  try {
    const hooksDir = join(homedir(), '.claude', 'hooks');
    mkdirSync(hooksDir, { recursive: true });

    // On Windows, use the .exe directly so Git Bash can execute it.
    const pluginBinDir = join(PLUGIN_ROOT, '.claude-plugin', 'bin');
    const cliCmd = IS_WIN ? join(pluginBinDir, 'context-mode.exe') : cliPath;

    for (const hookType of HOOK_TYPES) {
      const hookPath = join(hooksDir, `${hookType}${IS_WIN ? '.cmd' : '.sh'}`);
      let content;
      if (IS_WIN) {
        // Suppress all output except the JSON response
        content = `@echo off\r\n"${cliCmd}" hook claude-code ${hookType} %*\r\n`;
      } else {
        content = `#!/bin/sh\n"${cliCmd}" hook claude-code ${hookType} "$@"\n`;
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

function installSettingsHooks() {
  try {
    const hooksJsonPath = join(PLUGIN_ROOT, 'hooks', 'hooks.json');
    if (!existsSync(hooksJsonPath)) {
      log('hooks.json not found at', hooksJsonPath, 'skipping settings hooks');
      return;
    }

    const hooksJson = JSON.parse(readFileSync(hooksJsonPath, 'utf8'));
    if (!hooksJson.hooks) {
      log('No hooks field in hooks.json');
      return;
    }

    const settingsPath = join(homedir(), '.claude', 'settings.json');
    let settings = {};
    if (existsSync(settingsPath)) {
      try {
        settings = JSON.parse(readFileSync(settingsPath, 'utf8'));
      } catch (e) {
        log('Warning: could not parse existing settings.json, creating fresh one');
      }
    }

    // On Windows, rewrite commands to use absolute path to context-mode.exe
    // so shells (including Git Bash) can execute the binary directly.
    // Forward slashes avoid backslash-escaping issues in bash double quotes.
    const binDir = join(PLUGIN_ROOT, '.claude-plugin', 'bin');
    const cliCmd = IS_WIN
      ? join(binDir, 'context-mode.exe').replace(/\\/g, '/')
      : join(binDir, 'context-mode');
    const replacement = `"${cliCmd}" hook`;

    function rewriteCommands(obj) {
      if (Array.isArray(obj)) {
        for (const item of obj) rewriteCommands(item);
      } else if (obj && typeof obj === 'object') {
        for (const key of Object.keys(obj)) {
          if (key === 'command' && typeof obj[key] === 'string' && IS_WIN) {
            obj[key] = obj[key].replace(/context-mode hook/g, replacement);
          } else {
            rewriteCommands(obj[key]);
          }
        }
      }
    }
    rewriteCommands(hooksJson.hooks);

    // Skip write if hooks are already identical
    if (settings.hooks && JSON.stringify(settings.hooks) === JSON.stringify(hooksJson.hooks)) {
      log('Hooks already up to date in', settingsPath);
      return;
    }

    // Backup existing settings before mutation
    if (existsSync(settingsPath)) {
      const backupPath = settingsPath + '.bak';
      try {
        writeFileSync(backupPath, JSON.stringify(settings, null, 2));
      } catch (e) {
        log('Warning: could not create settings backup:', e.message);
      }
    }

    // Merge canonical context-mode hooks into settings
    settings.hooks = hooksJson.hooks;
    writeFileSync(settingsPath, JSON.stringify(settings, null, 2));
    log('Settings hooks installed in', settingsPath);
  } catch (e) {
    log('Settings hook install warning:', e.message);
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

    let insightBinary = findBinary(INSIGHT_BIN_NAME);
    if (!insightBinary) {
      insightBinary = await downloadBinary(INSIGHT_BIN_NAME, 'context-mode-insight');
    }

    if (cliBinary) {
      installSettingsHooks();
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
