import { after, afterEach, before, describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { spawn, spawnSync } from 'node:child_process';
import { mkdir, mkdtemp, rm, writeFile } from 'node:fs/promises';
import { createServer } from 'node:http';
import dgram from 'node:dgram';
import { createInterface } from 'node:readline';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { chromium } from 'playwright';
import { PNG } from 'pngjs';
import { closeBrowser, openPage } from '../browser.mjs';
import { bundleLitPackage } from '../lit-bundle.mjs';
import * as playground from '../playground.mjs';
import { ServiceError } from '../protocol.mjs';
import { descendantsOf, isAlive } from './helpers/process-family.mjs';
import { stageFixture } from './helpers/playground-stage.mjs';

const SERVICE = fileURLToPath(new URL('../service.mjs', import.meta.url));
const WINDOW = { width: 640, height: 360, scale: 1 };
const MAGENTA = [255, 0, 255];
const SETTLE_TIMEOUT_MS = 8_000;
const POLL_MS = 100;

const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

const launched = [];
const cdpCalls = [];
const launch = chromium.launch.bind(chromium);
chromium.launch = async (options) => {
  const browser = await launch(options);
  const newContext = browser.newContext.bind(browser);
  browser.newContext = async (contextOptions) => {
    const context = await newContext(contextOptions);
    const newCDPSession = context.newCDPSession.bind(context);
    context.newCDPSession = async (page) => {
      const session = await newCDPSession(page);
      const send = session.send.bind(session);
      session.send = (method, params) => {
        cdpCalls.push(method);
        return send(method, params);
      };
      return session;
    };
    return context;
  };
  launched.push({ options, browser });
  return browser;
};

const rejection = async (promise) => {
  const error = await promise.then(
    () => assert.fail('the call was expected to fail'),
    (caught) => caught,
  );
  assert.ok(error instanceof ServiceError, `expected a ServiceError, got ${String(error)}`);
  return error;
};

const currentBrowser = () => launched.at(-1).browser;
const currentPage = () => currentBrowser().contexts().at(-1).pages()[0];

async function rendererCount() {
  const cdp = await currentBrowser().newBrowserCDPSession();
  try {
    const { processInfo } = await cdp.send('SystemInfo.getProcessInfo');
    return processInfo.filter((entry) => entry.type === 'renderer').length;
  } finally {
    await cdp.detach();
  }
}

async function waitFor(check, message) {
  const deadline = Date.now() + SETTLE_TIMEOUT_MS;
  for (;;) {
    const value = await check();
    if (value) {
      return value;
    }
    assert.ok(Date.now() < deadline, message);
    await sleep(POLL_MS);
  }
}

const pixelOf = (png, [x, y]) => {
  const index = (png.width * y + x) * 4;
  return [png.data[index], png.data[index + 1], png.data[index + 2]];
};

describe('lot 7 review: playground open expiry', () => {
  const cleanups = [];

  const stage = async (files) => {
    const staged = await stageFixture(files);
    cleanups.push(staged.remove);
    return staged;
  };

  const open = async (files, options) => {
    const staged = await stage(files);
    return playground.open({ dir: staged.root, ...WINDOW, theme: 'light', tags: [] }, options);
  };

  afterEach(async () => {
    await playground.close();
    await Promise.all(cleanups.splice(0).map((cleanup) => cleanup()));
  });

  after(async () => {
    await closeBrowser();
  });

  it('closes the page of an open that outlived its budget so that no renderer is left behind', async () => {
    await open({});
    await playground.close();
    const baseline = await rendererCount();

    const error = await rejection(open({ 'lib/index.ts': `export {};\nfor (;;) {}\n` }, { budgetMs: 2_000 }));

    assert.equal(error.kind, 'render');
    await waitFor(async () => (await rendererCount()) <= baseline, `a renderer is still alive after the open expired (baseline ${baseline})`);
    await open({});
    assert.deepEqual(await playground.input({ events: [{ type: 'move', x: 5, y: 5 }] }), {});
  });

  it('never installs a session from an open that expired', async () => {
    const error = await rejection(
      open({ 'lib/index.ts': `export {};\nconst end = Date.now() + 5000;\nwhile (Date.now() < end) {}\n` }, { budgetMs: 1_500 }),
    );
    assert.equal(error.kind, 'render');

    await sleep(6_500);

    const absent = await rejection(playground.frame({ after: 0 }));
    assert.equal(absent.kind, 'invalidRequest');
    assert.match(absent.message, /no playground is open/);
  });
});

describe('lot 7 review: network hermeticity', () => {
  it('launches Chromium with no host resolution and no WebRTC route to the network', async () => {
    const { page, close } = await openPage({});
    await close();
    const { args } = launched.at(-1).options;
    assert.ok(args.includes('--host-resolver-rules=MAP * ~NOTFOUND'), JSON.stringify(args));
    assert.ok(args.includes('--force-webrtc-ip-handling-policy=disable_non_proxied_udp'), JSON.stringify(args));
    assert.ok(page);
  });

  it('establishes no WebSocket, no WebRTC traffic and no preconnect against a local server', async () => {
    const hits = [];
    const server = createServer((request, response) => {
      hits.push(`http ${request.url}`);
      response.end();
    });
    server.on('connection', () => hits.push('tcp'));
    server.on('upgrade', (request, socket) => {
      hits.push('upgrade');
      socket.destroy();
    });
    await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
    const udp = dgram.createSocket('udp4');
    udp.on('message', () => hits.push('udp'));
    await new Promise((resolve) => udp.bind(0, '127.0.0.1', resolve));
    const { port } = server.address();
    const { port: udpPort } = udp.address();
    const { page, close } = await openPage({});
    try {
      await page.setContent(
        `<link rel="preconnect" href="http://127.0.0.1:${port}"><link rel="dns-prefetch" href="http://localhost:${port}"><link rel="preconnect" href="http://localhost:${port}">`,
      );
      await page.evaluate(
        async ({ tcpPort, stunPort }) => {
          new WebSocket(`ws://127.0.0.1:${tcpPort}/socket`);
          const connection = new RTCPeerConnection({ iceServers: [{ urls: `stun:127.0.0.1:${stunPort}` }] });
          connection.createDataChannel('probe');
          await connection.setLocalDescription(await connection.createOffer());
        },
        { tcpPort: port, stunPort: udpPort },
      );
      await sleep(1_500);
      assert.deepEqual(hits, []);
    } finally {
      await close();
      await closeBrowser();
      udp.close();
      await new Promise((resolve) => server.close(resolve));
    }
  });
});

describe('lot 7 review: bundle of the project lit and path masking', () => {
  const cleanups = [];

  afterEach(async () => {
    await Promise.all(cleanups.splice(0).map((cleanup) => cleanup()));
  });

  it('bundles the lit of the service even when the project root holds its own node_modules/lit', async () => {
    const staged = await stageFixture();
    cleanups.push(staged.remove);
    const projectLit = path.join(staged.root, 'node_modules', 'lit');
    await mkdir(projectLit, { recursive: true });
    await writeFile(path.join(projectLit, 'package.json'), JSON.stringify({ name: 'lit', version: '0.0.1', main: 'index.js' }));
    await writeFile(path.join(projectLit, 'index.js'), `export const PROJECT_LIT_MARKER = 'project-lit-marker';\n`);
    await writeFile(path.join(projectLit, 'decorators.js'), `export const customElement = () => () => undefined;\nexport const state = () => () => undefined;\n`);

    const { code } = await bundleLitPackage({ libDir: staged.libDir });

    assert.ok(!code.includes('project-lit-marker'), 'the project lit must never enter the bundle');
    assert.ok(code.length > 1_000);
  });

  it('masks Windows and POSIX absolute paths in bundle errors', async () => {
    const staged = await stageFixture({
      'lib/components/ghost.ts': `import 'C:\\\\nowhere-win-dir\\\\x.js';\nimport '/nowhere-posix-dir/x.js';\n`,
      'lib/index.ts': `export * from './components/ghost.js';\n`,
    });
    cleanups.push(staged.remove);

    const windows = await rejection(bundleLitPackage({ libDir: staged.libDir }));
    assert.ok(!windows.message.includes('nowhere-win-dir'), windows.message);

    await writeFile(path.join(staged.libDir, 'components', 'ghost.ts'), `import '/nowhere-posix-dir/x.js';\n`);
    const posix = await rejection(bundleLitPackage({ libDir: staged.libDir }));
    assert.ok(!posix.message.includes('nowhere-posix-dir'), posix.message);
  });

  it('answers a stable render error without the machine path when a staged file cannot be read', async () => {
    const staged = await stageFixture();
    cleanups.push(staged.remove);
    const tokens = path.join(staged.libDir, 'tokens', 'tokens.css');
    await rm(tokens);
    await mkdir(tokens);

    const error = await rejection(playground.open({ dir: staged.root, ...WINDOW, theme: 'light', tags: [] }));

    assert.equal(error.kind, 'render');
    assert.ok(!error.message.includes(staged.root), error.message);
    assert.ok(!error.message.includes(os.tmpdir()), error.message);
    assert.match(error.message, /tokens\.css/);
  });
});

describe('lot 7 review: playground frames, settings and guard', () => {
  const cleanups = [];

  const open = async (files, params = {}) => {
    const staged = await stageFixture(files);
    cleanups.push(staged.remove);
    return { staged, result: await playground.open({ dir: staged.root, ...WINDOW, theme: 'light', tags: [], ...params }) };
  };

  const pngFrame = async () => {
    const frame = await waitFor(async () => {
      const response = await playground.frame({ after: 0, format: 'png' });
      return response.changed ? response : null;
    }, 'no image within the allowed time');
    return PNG.sync.read(Buffer.from(frame.data, 'base64'));
  };

  const waitForPixel = (point, expected, message) =>
    waitFor(async () => {
      const png = await pngFrame();
      return pixelOf(png, point).every((channel, index) => channel === expected[index]);
    }, message);

  afterEach(async () => {
    await playground.close();
    await Promise.all(cleanups.splice(0).map((cleanup) => cleanup()));
    cdpCalls.length = 0;
  });

  after(async () => {
    await closeBrowser();
  });

  it('answers the same tooLarge error for an oversized image without encoding it again', async () => {
    const noise = `const canvas = document.createElement('canvas');
canvas.width = 3840;
canvas.height = 2160;
canvas.style.cssText = 'position: fixed; left: 0; top: 0';
const context = canvas.getContext('2d');
const image = context.createImageData(3840, 2160);
for (let offset = 0; offset < image.data.length; offset += 65536) {
  crypto.getRandomValues(image.data.subarray(offset, offset + 65536));
}
context.putImageData(image, 0, 0);
document.body.append(canvas);
export {};
`;
    await open({ 'lib/index.ts': noise }, { width: 3840, height: 2160 });
    const refused = await waitFor(async () => {
      try {
        await playground.frame({ after: 0, format: 'png' });
        return null;
      } catch (error) {
        return error;
      }
    }, 'the oversized image was never refused');
    assert.equal(refused.kind, 'tooLarge');
    const encodings = cdpCalls.filter((method) => method === 'Page.captureScreenshot').length;

    for (let attempt = 0; attempt < 3; attempt += 1) {
      const again = await rejection(playground.frame({ after: 0, format: 'png' }));
      assert.equal(again.kind, 'tooLarge');
      assert.equal(again.message, refused.message);
    }

    assert.equal(cdpCalls.filter((method) => method === 'Page.captureScreenshot').length, encodings);
  });

  it('raises the image number after apply, setTemplate and input even when the picture barely changes', async () => {
    const { staged } = await open({});
    const first = await waitFor(async () => {
      const response = await playground.frame({ after: 0 });
      return response.changed ? response : null;
    }, 'no first image');
    const settled = await waitFor(async () => {
      const response = await playground.frame({ after: first.seq });
      return response.changed ? null : response;
    }, 'the image never settled');
    let seen = settled.seq;

    await playground.apply({ overrides: ':root[data-theme="light"] { --x-color-primary: rgb(200, 30, 31); }' });
    let next = await playground.frame({ after: seen });
    assert.equal(next.changed, true, 'apply must raise seq');
    seen = next.seq;

    await playground.setTemplate({ dir: staged.root });
    next = await playground.frame({ after: seen });
    assert.equal(next.changed, true, 'setTemplate must raise seq');
    seen = next.seq;

    await playground.input({ events: [{ type: 'move', x: 600, y: 300 }] });
    next = await playground.frame({ after: seen });
    assert.equal(next.changed, true, 'input must raise seq');
  });

  it('touches the color scheme and the shapes only when the theme or the shape changes, and follows late elements', async () => {
    const lateLib = `export * from './components/x-button.js';
class XLate extends HTMLElement {
  constructor() {
    super();
    this.attachShadow({ mode: 'open' }).innerHTML = '<i></i>';
  }
}
customElements.define('x-late', XLate);
`;
    await open({ 'lib/index.ts': lateLib });
    const page = currentPage();
    const evaluated = [];
    const emulated = [];
    const evaluate = page.evaluate.bind(page);
    const emulateMedia = page.emulateMedia.bind(page);
    page.evaluate = (work, argument) => {
      evaluated.push(work.name);
      return evaluate(work, argument);
    };
    page.emulateMedia = (options) => {
      emulated.push(options);
      return emulateMedia(options);
    };

    await playground.apply({ theme: 'light', shape: 'round', overrides: '' });
    assert.deepEqual(emulated, []);
    assert.ok(!evaluated.includes('applyShape'), JSON.stringify(evaluated));

    await playground.apply({ theme: 'dark', shape: 'squircle' });
    assert.deepEqual(emulated, [{ colorScheme: 'dark' }]);
    assert.equal(evaluated.filter((name) => name === 'applyShape').length, 1);

    await playground.apply({ theme: 'dark', shape: 'squircle' });
    assert.equal(emulated.length, 1);
    assert.equal(evaluated.filter((name) => name === 'applyShape').length, 1);

    page.evaluate = evaluate;
    const adopted = await page.evaluate(async () => {
      const wait = () => new Promise((resolve) => setTimeout(resolve, 150));
      const top = document.createElement('x-late');
      document.body.append(top);
      await wait();
      const nested = document.createElement('x-late');
      top.shadowRoot.append(nested);
      await wait();
      return [top, nested].map((element) => element.shadowRoot.adoptedStyleSheets.length);
    });
    assert.deepEqual(adopted, [1, 1]);
  });

  it('bounds the page errors it reports to 16 messages of 256 characters', async () => {
    const noisy = `export * from './components/x-button.js';
for (let index = 0; index < 40; index += 1) {
  queueMicrotask(() => {
    throw new Error('e'.repeat(1000));
  });
}
`;
    const { result } = await open({ 'lib/index.ts': noisy });

    assert.equal(result.errors.length, 16);
    assert.ok(result.errors.every((message) => message.length <= 256), JSON.stringify(result.errors.map((message) => message.length)));
  });

  it('keeps its page when an svg link or a link inside a closed shadow root is clicked', async () => {
    const server = createServer((request, response) => response.end('left the page'));
    await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
    cleanups.push(() => new Promise((resolve) => server.close(resolve)));
    const origin = `http://127.0.0.1:${server.address().port}`;
    const closedLib = `export * from './components/x-button.js';
class XClosed extends HTMLElement {
  constructor() {
    super();
    this.attachShadow({ mode: 'closed' }).innerHTML = '<a href="${origin}/closed"><div style="width: 100px; height: 30px; background: rgb(255, 0, 255)"></div></a>';
  }
}
customElements.define('x-closed', XClosed);
`;
    const template = `<svg width="100" height="30" style="position: fixed; left: 20px; top: 20px"><a href="${origin}/svg"><rect width="100" height="30" fill="rgb(255, 0, 255)"/></a></svg>
<x-closed style="position: fixed; left: 20px; top: 100px"></x-closed>
<a href="${origin}/plain" style="position: fixed; left: 20px; top: 180px; display: block; width: 100px; height: 30px; background: rgb(255, 0, 255)"></a>\n`;
    await open({ 'lib/index.ts': closedLib, 'template.html': template });
    await waitForPixel([30, 30], MAGENTA, 'the svg link never rendered');

    const clicks = [
      { x: 60, y: 35 },
      { x: 60, y: 115 },
      { x: 60, y: 195 },
    ];
    for (const { x, y } of clicks) {
      await playground.input({
        events: [
          { type: 'move', x, y },
          { type: 'down', x, y, button: 'left' },
          { type: 'up', x, y, button: 'left' },
        ],
      });
    }
    await sleep(600);

    for (const [x, y] of [
      [30, 30],
      [30, 110],
      [30, 190],
    ]) {
      await waitForPixel([x, y], MAGENTA, `the page left its content (pixel ${x},${y})`);
    }
  });
});

describe('lot 7 review: service stop', () => {
  const stray = [];

  after(() => {
    for (const pid of stray.splice(0)) {
      if (isAlive(pid)) {
        try {
          process.kill(pid);
        } catch {
          continue;
        }
      }
    }
  });

  it('exits within a bounded time when a page loops and stdin ends', async () => {
    const staged = await stageFixture({
      'template.html': `<div onclick="for (;;) {}" style="position: fixed; left: 20px; top: 20px; width: 100px; height: 30px; background: rgb(255, 0, 255)"></div>\n`,
    });
    const child = spawn(process.execPath, [SERVICE], { stdio: ['pipe', 'pipe', 'pipe'], windowsHide: true });
    const exited = new Promise((resolve) => child.once('exit', resolve));
    const lines = createInterface({ input: child.stdout })[Symbol.asyncIterator]();
    const call = async (id, method, params) => {
      child.stdin.write(`${JSON.stringify({ id, method, params })}\n`);
      for (;;) {
        const { value } = await lines.next();
        const message = JSON.parse(value);
        if (message.id === id) {
          return message;
        }
      }
    };
    try {
      await lines.next();
      const opened = await call(1, 'playground.open', { dir: staged.root, ...WINDOW, theme: 'light', tags: [] });
      assert.equal(opened.ok, true, JSON.stringify(opened));
      stray.push(...descendantsOf(child.pid));
      const point = { x: 60, y: 35 };
      child.stdin.write(
        `${JSON.stringify({ id: 2, method: 'playground.input', params: { events: [{ type: 'move', ...point }, { type: 'down', ...point, button: 'left' }, { type: 'up', ...point, button: 'left' }] } })}\n`,
      );
      await sleep(1_000);
      const started = Date.now();

      child.stdin.end();
      const outcome = await Promise.race([exited.then(() => 'exited'), sleep(25_000).then(() => 'still running')]);

      assert.equal(outcome, 'exited', 'the service must stop although a page loops');
      assert.ok(Date.now() - started < 25_000);
      await waitFor(() => stray.every((pid) => !isAlive(pid)), 'a Chromium process outlived the service');
    } finally {
      if (child.exitCode === null && child.signalCode === null) {
        spawnSync('taskkill', ['/T', '/F', '/PID', String(child.pid)], { windowsHide: true });
        await exited;
      }
      await staged.remove();
    }
  });
});
