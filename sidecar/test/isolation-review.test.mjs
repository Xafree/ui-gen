import { after, afterEach, describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { createServer } from 'node:http';
import { symlink, unlink, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { chromium } from 'playwright';
import { closeBrowser, withPage } from '../browser.mjs';
import { render } from '../render.mjs';
import { ServiceError } from '../protocol.mjs';
import { makeSourcesProject, makeTempDir, writePackage } from './helpers/review-project.mjs';

const SECRET = 'UIGEN_SECRET_MARKER_4f9c1e';

const renderParams = (dir, name, component) => ({ dir, package: name, component, props: {}, state: 'default', theme: 'light' });

const leakingComponent = (importLine) => `import { createElement } from 'react';
${importLine}
export function Leak() { return createElement('p', null, secret); }
`;

const rejectsAsRender = async (promise) => {
  const error = await promise.then(
    () => assert.fail('the render was expected to fail'),
    (caught) => caught,
  );
  assert.ok(error instanceof ServiceError, `expected a ServiceError, got ${String(error)}`);
  assert.equal(error.kind, 'render');
  assert.ok(!error.message.includes(SECRET), 'the refused file content must not reach the response');
  assert.ok(error.message.includes('outside'), error.message);
};

describe('bundle confinement', () => {
  const cleanups = [];

  afterEach(async () => {
    await Promise.all(cleanups.splice(0).map((cleanup) => cleanup()));
  });

  after(closeBrowser);

  const track = (project) => {
    cleanups.push(project.remove);
    return project;
  };

  it('refuses a relative import that climbs out of .uigen/sources', async () => {
    const project = track(await makeSourcesProject({ escaper: { 'index.js': leakingComponent("import secret from '../../../../outside.js';") } }));
    await writeFile(path.join(project.dir, 'outside.js'), `export default '${SECRET}';\n`);

    await rejectsAsRender(render(renderParams(project.dir, 'escaper', 'Leak')));
  });

  it('refuses an absolute import of a file outside the project', async () => {
    const outside = track(await makeTempDir('uigen-outside-'));
    const file = path.join(outside.dir, 'secret.js');
    await writeFile(file, `export default '${SECRET}';\n`);
    const project = track(await makeSourcesProject({ absolute: { 'index.js': leakingComponent(`import secret from ${JSON.stringify(file)};`) } }));

    await rejectsAsRender(render(renderParams(project.dir, 'absolute', 'Leak')));
  });

  it('refuses a package folder that is a link to a folder outside the project', async () => {
    const outside = track(await makeTempDir('uigen-outside-'));
    await writePackage(outside.dir, 'linked', { 'index.js': leakingComponent(`const secret = '${SECRET}';`) });
    const project = track(await makeSourcesProject({}));
    const link = path.join(project.modules, 'linked');
    await symlink(path.join(outside.dir, 'linked'), link, 'junction');
    cleanups.unshift(() => unlink(link).catch(() => undefined));

    await rejectsAsRender(render(renderParams(project.dir, 'linked', 'Leak')));
  });
});

describe('page isolation', () => {
  after(closeBrowser);

  it('cannot reach the network from the page', async () => {
    const hits = [];
    const server = createServer((request, response) => {
      hits.push(request.url);
      response.setHeader('access-control-allow-origin', '*');
      response.end('reached');
    });
    await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
    const origin = `http://127.0.0.1:${server.address().port}`;
    try {
      const outcome = await withPage({}, async (page) => {
        await page.setContent(`<img src="${origin}/image">`);
        const fetched = await page.evaluate(
          (target) =>
            fetch(`${target}/fetch`).then(
              () => 'reached',
              () => 'blocked',
            ),
          origin,
        );
        const local = await page.evaluate(() => fetch('data:text/plain,local').then((response) => response.text()));
        const navigated = await page.goto(`${origin}/navigation`).then(
          () => 'reached',
          () => 'blocked',
        );
        return { fetched, navigated, local };
      });

      assert.deepEqual(outcome, { fetched: 'blocked', navigated: 'blocked', local: 'local' });
      assert.deepEqual(hits, []);
    } finally {
      await new Promise((resolve) => server.close(resolve));
    }
  });
});

describe('browser launch', () => {
  const realLaunch = chromium.launch;

  afterEach(async () => {
    chromium.launch = realLaunch;
    await closeBrowser();
  });

  it('launches Chromium with its OS sandbox and no debugging port', async () => {
    await closeBrowser();
    const launches = [];
    chromium.launch = (options) => {
      launches.push(options);
      return realLaunch.call(chromium, options);
    };

    await withPage({}, async (page) => page.evaluate(() => 1));

    assert.equal(launches.length, 1);
    assert.equal(launches[0].chromiumSandbox, true);
    assert.ok(launches[0].args.includes('--disable-lcd-text'));
    assert.ok(!launches[0].args.some((arg) => arg.startsWith('--remote-debugging')));
  });

  it('launches Chromium again after a launch failure', async () => {
    await closeBrowser();
    let attempts = 0;
    chromium.launch = (options) => {
      attempts += 1;
      return attempts === 1 ? Promise.reject(new Error('launch failed once')) : realLaunch.call(chromium, options);
    };

    await assert.rejects(withPage({}, async () => 'unreachable'), /launch failed once/);
    const value = await withPage({}, async (page) => page.evaluate(() => 41 + 1));

    assert.equal(value, 42);
    assert.equal(attempts, 2);
  });
});
