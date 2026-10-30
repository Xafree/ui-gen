import { after, afterEach, describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { existsSync } from 'node:fs';
import { mkdtemp, rm } from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { pathToFileURL } from 'node:url';
import { closeBrowser, loadBundle, withPage } from '../browser.mjs';
import { bundleLitPackage } from '../lit-bundle.mjs';
import { ServiceError } from '../protocol.mjs';
import { FIXTURE_TRIPWIRE, stageFixture } from './helpers/playground-stage.mjs';

const SECRET = 'UIGEN_SECRET_MARKER_7b2d90';

const INDEX_WITH = (extraImport) => `import './tripwire.js';
export * from './components/x-button.js';
${extraImport}
`;

const rejection = async (promise) => {
  const error = await promise.then(
    () => assert.fail('the bundle was expected to fail'),
    (caught) => caught,
  );
  assert.ok(error instanceof ServiceError, `expected a ServiceError, got ${String(error)}`);
  return error;
};

describe('lit package bundle', () => {
  const cleanups = [];
  const stage = async (files) => {
    const staged = await stageFixture(files);
    cleanups.push(staged.remove);
    return staged;
  };

  afterEach(async () => {
    delete process.env.UIGEN_TRIPWIRE;
    await Promise.all(cleanups.splice(0).map((cleanup) => cleanup()));
  });

  after(closeBrowser);

  // covers: E7-1.1
  it('bundles the synthetic package into a script that defines x-button with working decorators in Chromium', async () => {
    const { libDir } = await stage();

    const { code } = await bundleLitPackage({ libDir });

    assert.equal(typeof code, 'string');
    assert.ok(code.includes('x-button'));
    const outcome = await withPage({ viewport: { width: 400, height: 300 } }, async (page) => {
      const errors = await loadBundle(page, { code, html: '<!doctype html><html><body><x-button>Hi</x-button></body></html>' });
      const probe = await page.evaluate(async () => {
        const element = document.querySelector('x-button');
        element.click();
        await element.updateComplete;
        const marker = element.shadowRoot.querySelector('.clicks');
        return {
          defined: customElements.get('x-button') !== undefined,
          hasButton: element.shadowRoot.querySelector('button') !== null,
          clickMarker: getComputedStyle(marker).backgroundColor,
        };
      });
      return { errors, probe };
    });
    assert.deepEqual(outcome.errors, []);
    assert.deepEqual(outcome.probe, { defined: true, hasButton: true, clickMarker: 'rgb(0, 50, 0)' });
  });

  // covers: E7-1.9
  it('refuses an import that climbs out of lib/ and names the file and the cause without leaking the target', async () => {
    const outside = await stage({ 'secret.js': `export default '${SECRET}';\n` });
    const { libDir, staging } = await stage({
      'lib/components/leak.ts': `import secret from '../../secret.js';\nexport const leak = secret;\n`,
      'lib/index.ts': INDEX_WITH(`import './components/leak.js';`),
      'secret.js': `export default '${SECRET}';\n`,
    });

    const relative = await rejection(bundleLitPackage({ libDir }));
    assert.equal(relative.kind, 'render');
    assert.ok(relative.message.includes('components/leak.ts'), relative.message);
    assert.ok(relative.message.includes('outside'), relative.message);
    assert.ok(!relative.message.includes(SECRET), 'the refused file content must not reach the message');
    assert.ok(!relative.message.includes(staging), 'the message must not contain an absolute path of the machine');

    const absolute = await rejection(
      bundleLitPackage({
        libDir: (
          await stage({
            'lib/components/leak.ts': `import secret from ${JSON.stringify(path.join(outside.staging, 'secret.js'))};\nexport const leak = secret;\n`,
            'lib/index.ts': INDEX_WITH(`import './components/leak.js';`),
          })
        ).libDir,
      }),
    );
    assert.equal(absolute.kind, 'render');
    assert.ok(absolute.message.includes('components/leak.ts'), absolute.message);
    assert.ok(absolute.message.includes('outside'), absolute.message);
    assert.ok(!absolute.message.includes(SECRET));
  });

  // covers: E7-1.9
  it('answers render with the file and the cause when a module cannot be found', async () => {
    const { libDir } = await stage({
      'lib/components/ghost.ts': `import './nowhere-to-be-found.js';\n`,
      'lib/index.ts': INDEX_WITH(`import './components/ghost.js';`),
    });

    const error = await rejection(bundleLitPackage({ libDir }));

    assert.equal(error.kind, 'render');
    assert.ok(error.message.includes('components/ghost.ts'), error.message);
    assert.ok(error.message.includes('nowhere-to-be-found'), error.message);
  });

  // covers: E7-1.9
  it('answers notInstalled when index.ts is absent', async () => {
    const { libDir } = await stage({ 'lib/index.ts': null });

    const error = await rejection(bundleLitPackage({ libDir }));

    assert.equal(error.kind, 'notInstalled');
  });

  // covers: E7-1.9
  it('never executes the code of the library in Node', async () => {
    const control = path.join(await mkdtemp(path.join(os.tmpdir(), 'uigen-tripwire-')), 'control.txt');
    cleanups.push(() => rm(path.dirname(control), { recursive: true, force: true }));
    process.env.UIGEN_TRIPWIRE = control;
    await import(pathToFileURL(FIXTURE_TRIPWIRE).href);
    assert.ok(existsSync(control), 'the tripwire fires when Node runs it');

    const fired = path.join(path.dirname(control), 'fired.txt');
    process.env.UIGEN_TRIPWIRE = fired;
    const { libDir } = await stage();
    await bundleLitPackage({ libDir });

    assert.ok(!existsSync(fired), 'bundling must not run the library in Node');
  });
});
