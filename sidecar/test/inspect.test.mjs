import { after, before, describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { makeProject, startService } from './helpers/service-client.mjs';

describe('inspect', () => {
  let project;
  let service;

  before(async () => {
    project = await makeProject(['synthetic-ui']);
    service = await startService();
  });

  after(async () => {
    await service?.stop();
    await project?.remove();
  });

  // covers: E2-3.1
  it('lists the React Button and the x-chip web component of an installed package with their typed props', async () => {
    const response = await service.request('inspect', { dir: project.dir, package: 'synthetic-ui' });

    assert.equal(response.ok, true, JSON.stringify(response));
    assert.equal(response.result.package, 'synthetic-ui');
    assert.equal(response.result.version, '1.0.0');
    assert.equal(response.result.framework, 'react');
    const names = response.result.components.map((component) => component.name).sort();
    assert.deepEqual(names, ['Button', 'x-chip']);
    const props = Object.fromEntries(response.result.components.map((component) => [component.name, component.props]));
    assert.deepEqual(props.Button, [
      { name: 'variant', type: 'enum', values: ['solid', 'outline'] },
      { name: 'size', type: 'enum', values: ['sm', 'md'] },
      { name: 'disabled', type: 'boolean', values: null },
    ]);
    assert.deepEqual(props['x-chip'], [{ name: 'label', type: 'string', values: null }]);
  });

  // covers: E2-3.3
  it('answers notInstalled and names the package when it is not installed', async () => {
    const response = await service.request('inspect', { dir: project.dir, package: 'not-installed-pkg' });

    assert.equal(response.ok, false);
    assert.equal(response.error.kind, 'notInstalled');
    assert.ok(response.error.message.includes('not-installed-pkg'), response.error.message);
  });
});
