import { after, afterEach, before, describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { existsSync } from 'node:fs';
import { mkdtemp, rm, writeFile } from 'node:fs/promises';
import { createServer } from 'node:http';
import os from 'node:os';
import path from 'node:path';
import { PNG } from 'pngjs';
import { killRenderersUnder } from './helpers/kill-renderers.mjs';
import { stageFixture } from './helpers/playground-stage.mjs';
import { startService } from './helpers/service-client.mjs';

const WINDOW = { width: 640, height: 360, scale: 1 };
const SECRET = 'UIGEN_SECRET_MARKER_7b2d90';
const RED = [200, 30, 30];
const DARK_BLUE = [30, 60, 200];
const BLUE = [0, 0, 255];
const GREEN = [0, 200, 0];
const MAGENTA = [255, 0, 255];
const WHITE = [255, 255, 255];
const BLACK = [0, 0, 0];
const YELLOW = [255, 255, 0];
const CLICKED = [0, 50, 0];
const LOADED_ONCE = [50, 0, 0];
const PRIMARY = [30, 28];
const HOVER_EDGE = [22, 40];
const CLICK_MARKER = [24, 68];
const LOAD_MARKER = [36, 68];
const BUTTON_CENTER = { x: 80, y: 40 };
const AWAY = { x: 600, y: 300 };
const SETTLE_TIMEOUT_MS = 6_000;
const POLL_MS = 100;

const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

const light = (declarations) => `:root[data-theme="light"] { ${declarations} }`;
const dark = (declarations) => `:root[data-theme="dark"] { ${declarations} }`;

const clickAt = ({ x, y }) => [
  { type: 'move', x, y },
  { type: 'down', x, y, button: 'left' },
  { type: 'up', x, y, button: 'left' },
];

const jpegSize = (bytes) => {
  for (let offset = 2; offset + 9 < bytes.length; ) {
    assert.equal(bytes[offset], 0xff, 'JPEG marker');
    const marker = bytes[offset + 1];
    if (marker >= 0xc0 && marker <= 0xcf && ![0xc4, 0xc8, 0xcc].includes(marker)) {
      return { height: bytes.readUInt16BE(offset + 5), width: bytes.readUInt16BE(offset + 7) };
    }
    offset += 2 + bytes.readUInt16BE(offset + 2);
  }
  return assert.fail('no JPEG frame header');
};

const pixelOf = (png, [x, y]) => {
  const index = (png.width * y + x) * 4;
  return [png.data[index], png.data[index + 1], png.data[index + 2]];
};

describe('playground page', () => {
  let service;
  let tripwireFile;
  const cleanups = [];

  const call = (method, params = {}) => service.request(method, params);

  const succeeds = async (method, params) => {
    const response = await call(method, params);
    assert.equal(response.ok, true, `${method}: ${JSON.stringify(response)}`);
    return response.result;
  };

  const fails = async (method, params, kind) => {
    const response = await call(method, params);
    assert.equal(response.ok, false, `${method} was expected to fail`);
    assert.equal(response.error.kind, kind, JSON.stringify(response.error));
    return response.error;
  };

  const openStaged = async (files, overrides) => {
    const staged = await stageFixture(files);
    cleanups.push(staged.remove);
    const result = await succeeds('playground.open', { dir: staged.root, ...WINDOW, theme: 'light', tags: ['x-button'], ...overrides });
    return { ...staged, result };
  };

  const grab = async (format) => {
    const deadline = Date.now() + SETTLE_TIMEOUT_MS;
    for (;;) {
      const frame = await succeeds('playground.frame', { after: 0, ...(format === undefined ? {} : { format }) });
      if (frame.changed) {
        return frame;
      }
      assert.ok(Date.now() < deadline, 'no image within the allowed time');
      await sleep(POLL_MS);
    }
  };

  const expectPixels = async (expected) => {
    const deadline = Date.now() + SETTLE_TIMEOUT_MS;
    for (;;) {
      const frame = await grab('png');
      const png = PNG.sync.read(Buffer.from(frame.data, 'base64'));
      const actual = expected.map(([point]) => pixelOf(png, point));
      if (expected.every(([, color], index) => actual[index].every((channel, i) => channel === color[i]))) {
        return;
      }
      if (Date.now() >= deadline) {
        assert.deepEqual(
          actual,
          expected.map(([, color]) => color),
          `pixels at ${JSON.stringify(expected.map(([point]) => point))}`,
        );
      }
      await sleep(POLL_MS);
    }
  };

  const nextFrameAfter = async (seq) => {
    const deadline = Date.now() + SETTLE_TIMEOUT_MS;
    for (;;) {
      const frame = await succeeds('playground.frame', { after: seq });
      if (frame.changed) {
        return frame;
      }
      assert.ok(Date.now() < deadline, `no image newer than ${seq}`);
      await sleep(POLL_MS);
    }
  };

  before(async () => {
    tripwireFile = path.join(await mkdtemp(path.join(os.tmpdir(), 'uigen-tripwire-')), 'fired.txt');
    process.env.UIGEN_TRIPWIRE = tripwireFile;
    service = await startService();
    delete process.env.UIGEN_TRIPWIRE;
  });

  afterEach(async () => {
    await Promise.all(cleanups.splice(0).map((cleanup) => cleanup()));
  });

  after(async () => {
    await service?.request('playground.close', {}).catch(() => undefined);
    await service?.stop();
    await rm(path.dirname(tripwireFile), { recursive: true, force: true });
  });

  // covers: E7-1.1
  it('opens the synthetic package and serves a JPEG of the requested size', async () => {
    const { result } = await openStaged({}, { width: 480, height: 300, scale: 2 });

    assert.ok(result.defined.includes('x-button'), JSON.stringify(result));
    assert.deepEqual(result.missing, []);
    assert.deepEqual(result.errors, []);
    assert.equal(typeof result.shape, 'boolean');
    const frame = await grab();
    assert.equal(frame.format, 'jpeg');
    const bytes = Buffer.from(frame.data, 'base64');
    assert.deepEqual([...bytes.subarray(0, 2)], [0xff, 0xd8]);
    assert.deepEqual({ width: frame.width, height: frame.height }, { width: 960, height: 600 });
    assert.deepEqual(jpegSize(bytes), { width: 960, height: 600 });
  });

  // covers: E7-1.2
  it('lists an expected tag the package does not define in missing and keeps the page open', async () => {
    const { result } = await openStaged({}, { tags: ['x-button', 'x-ghost'] });

    assert.deepEqual(result.missing, ['x-ghost']);
    assert.ok(result.defined.includes('x-button'));
    await expectPixels([[PRIMARY, RED]]);
  });

  // covers: E7-1.3
  it('applies a token override without reloading the page or recreating the instance', async () => {
    await openStaged();
    await expectPixels([
      [PRIMARY, RED],
      [CLICK_MARKER, BLACK],
      [LOAD_MARKER, LOADED_ONCE],
    ]);
    await succeeds('playground.input', { events: [...clickAt(BUTTON_CENTER), { type: 'move', ...AWAY }] });
    await expectPixels([[CLICK_MARKER, CLICKED]]);

    await succeeds('playground.apply', { overrides: light('--x-color-primary: rgb(0, 0, 255);') });

    await expectPixels([
      [PRIMARY, BLUE],
      [CLICK_MARKER, CLICKED],
      [LOAD_MARKER, LOADED_ONCE],
    ]);
  });

  // covers: E7-1.4
  it('lets the dark override beat the dark media block and switches data-theme with the color scheme', async () => {
    await openStaged({}, { theme: 'dark' });
    await expectPixels([[PRIMARY, DARK_BLUE]]);

    await succeeds('playground.apply', { overrides: dark('--x-color-primary: rgb(0, 200, 0);') });
    await expectPixels([[PRIMARY, GREEN]]);

    await succeeds('playground.apply', { theme: 'light' });
    await expectPixels([[PRIMARY, RED]]);

    await succeeds('playground.apply', { theme: 'dark' });
    await expectPixels([[PRIMARY, GREEN]]);
  });

  // covers: E7-1.5
  it('replaces the body with the template without redefining the elements or losing the sheets', async () => {
    const { root, staging } = await openStaged();
    await succeeds('playground.input', { events: [...clickAt(BUTTON_CENTER), { type: 'move', ...AWAY }] });
    await expectPixels([[CLICK_MARKER, CLICKED]]);
    await writeFile(
      path.join(staging, 'template.html'),
      `<div style="position: fixed; left: 20px; top: 120px"><x-button>Moved</x-button></div>
<div style="position: fixed; left: 300px; top: 20px; width: 30px; height: 30px; background: rgb(0, 0, 255)"></div>\n`,
    );

    await succeeds('playground.setTemplate', { dir: root });

    await expectPixels([
      [PRIMARY, WHITE],
      [[30, 128], RED],
      [[310, 30], BLUE],
      [[24, 168], BLACK],
      [[36, 168], LOADED_ONCE],
    ]);
  });

  // covers: E7-1.5
  it('puts corner-shape in the shadow of x-button when open reports shape support', async (t) => {
    const { result } = await openStaged();
    if (!result.shape) {
      t.skip('this Chromium does not know corner-shape');
      return;
    }
    await succeeds('playground.apply', { overrides: light('--x-radius: 20px;') });
    await expectPixels([
      [[24, 24], WHITE],
      [PRIMARY, RED],
    ]);

    await succeeds('playground.apply', { shape: 'squircle' });

    await expectPixels([[[24, 24], RED]]);
  });

  // covers: E7-1.6
  it('relays hover and click to the page', async () => {
    await openStaged();
    await expectPixels([
      [HOVER_EDGE, RED],
      [CLICK_MARKER, BLACK],
    ]);

    await succeeds('playground.input', { events: [{ type: 'move', ...BUTTON_CENTER }] });
    await expectPixels([[HOVER_EDGE, YELLOW]]);
    await succeeds('playground.input', { events: [{ type: 'move', ...AWAY }] });
    await expectPixels([[HOVER_EDGE, RED]]);

    const accepted = await succeeds('playground.input', {
      events: [
        { type: 'wheel', x: 300, y: 200, dx: 0, dy: 10 },
        { type: 'key', key: 'ArrowDown' },
        { type: 'text', text: 'ab' },
        { type: 'key', key: 'Tab', mods: ['shift'] },
      ],
    });
    assert.deepEqual(accepted, {});
    await succeeds('playground.input', { events: clickAt(BUTTON_CENTER) });
    await expectPixels([[CLICK_MARKER, CLICKED]]);
  });

  // covers: E7-1.6
  it('refuses invalid input events as invalidRequest and executes none of the batch', async () => {
    await openStaged();
    const refused = [
      [{ type: 'key', key: 'F13' }],
      [{ type: 'move', x: WINDOW.width + 100, y: 10 }],
      [{ type: 'move', x: 10, y: -5 }],
      [{ type: 'move', x: '10', y: 10 }],
      [{ type: 'text', text: 'a'.repeat(257) }],
      [{ type: 'text', text: 'a\u0007b' }],
    ];

    for (const invalid of refused) {
      await fails('playground.input', { events: [...clickAt(BUTTON_CENTER), ...invalid] }, 'invalidRequest');
    }
    const sixtyFive = Array.from({ length: 65 }, (_, index) => ({ type: index % 2 === 0 ? 'down' : 'up', ...BUTTON_CENTER, button: 'left' }));
    await fails('playground.input', { events: sixtyFive }, 'invalidRequest');
    await fails('playground.input', { events: [] }, 'invalidRequest');
    await sleep(300);
    await expectPixels([[CLICK_MARKER, BLACK]]);
  });

  // covers: E7-1.7
  it('answers changed false when nothing moved and a newer image after a change', async () => {
    await openStaged();
    const first = await grab();

    const unchanged = await succeeds('playground.frame', { after: first.seq });
    assert.equal(unchanged.changed, false);

    await succeeds('playground.apply', { overrides: light('--x-color-primary: rgb(0, 0, 255);') });
    const next = await nextFrameAfter(first.seq);

    assert.ok(next.seq > first.seq);
  });

  // covers: E7-1.7
  it('produces no image between two calls when nobody asks for one', async () => {
    await openStaged({
      'template.html': `<style>@keyframes slide { to { transform: translateX(300px); } }</style>
<div style="position: fixed; left: 0; top: 100px; width: 50px; height: 50px; background: rgb(255, 0, 0); animation: slide 1s linear infinite alternate"></div>\n`,
    });
    const first = await grab();
    await sleep(700);

    const next = await nextFrameAfter(first.seq);

    assert.ok(next.seq <= first.seq + 2, `seq jumped from ${first.seq} to ${next.seq} while nobody was asking`);
  });

  // covers: E7-1.8
  it('sends no remote request, opens no window and stays on its page when links and images are used', async () => {
    const hits = [];
    const server = createServer((request, response) => {
      hits.push(request.url);
      response.end();
    });
    await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
    cleanups.push(() => new Promise((resolve) => server.close(resolve)));
    const origin = `http://127.0.0.1:${server.address().port}`;
    await openStaged({
      'template.html': `<img src="${origin}/a.png" width="30" height="30" style="position: fixed; left: 300px; top: 20px">
<a href="${origin}/page" style="position: fixed; left: 20px; top: 20px; width: 100px; height: 30px; background: rgb(0, 0, 255)">link</a>
<a href="https://example.invalid/" style="position: fixed; left: 20px; top: 60px; width: 100px; height: 30px; background: rgb(0, 200, 0)">external</a>
<div onclick="window.open('${origin}/popup')" style="position: fixed; left: 20px; top: 100px; width: 100px; height: 30px; background: rgb(255, 0, 255)"></div>\n`,
    });

    await succeeds('playground.input', { events: [...clickAt({ x: 60, y: 35 }), ...clickAt({ x: 60, y: 75 }), ...clickAt({ x: 60, y: 115 })] });
    await sleep(500);

    assert.deepEqual(hits, []);
    await expectPixels([
      [[25, 25], BLUE],
      [[25, 65], GREEN],
      [[25, 105], MAGENTA],
    ]);
  });

  // covers: E7-1.9
  it('answers render with the relative file and the cause for a library that imports outside lib/', async () => {
    const leaking = await stageFixture({
      'lib/components/leak.ts': `import secret from '../../secret.js';\nexport const leak = secret;\n`,
      'lib/index.ts': `import './tripwire.js';\nexport * from './components/x-button.js';\nimport './components/leak.js';\n`,
      'secret.js': `export default '${SECRET}';\n`,
    });
    cleanups.push(leaking.remove);

    const error = await fails('playground.open', { dir: leaking.root, ...WINDOW, theme: 'light', tags: [] }, 'render');

    assert.ok(error.message.includes('components/leak.ts'), error.message);
    assert.ok(error.message.includes('outside'), error.message);
    assert.ok(!error.message.includes(SECRET));
    assert.ok(!error.message.includes(leaking.root), 'the message must not contain an absolute path of the machine');
  });

  // covers: E7-1.9
  it('answers notInstalled when lib/index.ts is absent and never runs the library in Node', async () => {
    const missing = await stageFixture({ 'lib/index.ts': null });
    cleanups.push(missing.remove);

    await fails('playground.open', { dir: missing.root, ...WINDOW, theme: 'light', tags: [] }, 'notInstalled');

    await openStaged();
    await expectPixels([[PRIMARY, RED]]);
    assert.ok(!existsSync(tripwireFile), 'the library must be bundled, never executed by Node');
  });

  // covers: E7-1.10
  it('closes idempotently and frees the session so that open succeeds again', async () => {
    await openStaged();

    assert.deepEqual(await succeeds('playground.close', {}), {});
    assert.deepEqual(await succeeds('playground.close', {}), {});

    await openStaged();
    await expectPixels([[PRIMARY, RED]]);
  });

  // covers: E7-1.10
  it('replaces the previous session when open is called on an open playground', async () => {
    await openStaged();
    await expectPixels([[PRIMARY, RED]]);

    await openStaged({ 'template.html': `<div style="position: fixed; left: 200px; top: 20px; width: 30px; height: 30px; background: rgb(0, 0, 255)"></div>\n` });

    await expectPixels([
      [[210, 30], BLUE],
      [PRIMARY, WHITE],
    ]);
  });

  // covers: E7-1.10
  it('answers render "the playground page crashed" after a crash, until the next open', async () => {
    await openStaged();
    await expectPixels([[PRIMARY, RED]]);

    assert.ok(killRenderersUnder(service.pid) >= 1, 'a renderer process was killed');

    const deadline = Date.now() + SETTLE_TIMEOUT_MS * 2;
    let response = await call('playground.frame', { after: 0 });
    while (response.ok || response.error.kind !== 'render') {
      assert.ok(Date.now() < deadline, 'the crash was never reported');
      await sleep(POLL_MS);
      response = await call('playground.frame', { after: 0 });
    }
    assert.equal(response.error.kind, 'render');
    assert.ok(response.error.message.includes('the playground page crashed'), response.error.message);
    const again = await fails('playground.input', { events: [{ type: 'move', ...AWAY }] }, 'render');
    assert.ok(again.message.includes('the playground page crashed'), again.message);

    await openStaged();
    await expectPixels([[PRIMARY, RED]]);
  });
});
