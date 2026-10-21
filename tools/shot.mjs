// Headless-Chrome screenshot helper (verification only, not part of the app).
// Usage: node tools/shot.mjs <url> <out.png> "<js expression>" [width] [height]
// Drives Chrome over the DevTools protocol using Node's built-in WebSocket,
// so it needs no extra packages.
import { spawn } from 'node:child_process';
import { mkdirSync, writeFileSync } from 'node:fs';
import { setTimeout as sleep } from 'node:timers/promises';

const [url, out, expr = '1', width = '1200', height = '800'] = process.argv.slice(2);
const PORT = 9333 + Math.floor(Math.random() * 300);
const CHROME = 'C:/Program Files/Google/Chrome/Application/chrome.exe';

const chrome = spawn(CHROME, [
  '--headless=new', '--disable-gpu', '--hide-scrollbars', '--no-first-run',
  '--disable-extensions', '--force-device-scale-factor=1',
  `--remote-debugging-port=${PORT}`,
  `--user-data-dir=${process.env.TEMP || '/tmp'}/isle-shot-${PORT}`,
  `--window-size=${width},${height}`,
  'about:blank',
], { stdio: 'ignore' });

async function target() {
  for (let i = 0; i < 60; i++) {
    try {
      const r = await fetch(`http://127.0.0.1:${PORT}/json/list`);
      const list = await r.json();
      const page = list.find((t) => t.type === 'page');
      if (page?.webSocketDebuggerUrl) return page.webSocketDebuggerUrl;
    } catch { /* not up yet */ }
    await sleep(200);
  }
  throw new Error('Chrome devtools endpoint never appeared');
}

const ws = new WebSocket(await target());
await new Promise((res, rej) => { ws.onopen = res; ws.onerror = rej; });
let id = 0;
const pending = new Map();
ws.onmessage = (e) => {
  const m = JSON.parse(e.data);
  if (m.id && pending.has(m.id)) {
    const { res, rej } = pending.get(m.id);
    pending.delete(m.id);
    m.error ? rej(new Error(JSON.stringify(m.error))) : res(m.result);
  }
};
const send = (method, params = {}) => new Promise((res, rej) => {
  const n = ++id;
  pending.set(n, { res, rej });
  ws.send(JSON.stringify({ id: n, method, params }));
});

await send('Page.enable');
await send('Emulation.setDeviceMetricsOverride', {
  width: +width, height: +height, deviceScaleFactor: 1, mobile: false,
});
await send('Page.navigate', { url });
await sleep(1500);
const evaluated = await send('Runtime.evaluate', {
  expression: `(async () => { ${expr} })()`,
  awaitPromise: true, returnByValue: true,
});
const shot = await send('Page.captureScreenshot', { format: 'png', captureBeyondViewport: false });
mkdirSync(new URL('../screenshots/', import.meta.url), { recursive: true });
writeFileSync(out, Buffer.from(shot.data, 'base64'));
console.log(JSON.stringify({ out, value: evaluated?.result?.value ?? null }));
ws.close();
chrome.kill();
process.exit(0);
