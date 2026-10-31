import { afterEach, describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { cp, mkdir, mkdtemp, readFile, rm } from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { install } from '../install.mjs';
import { ServiceError } from '../protocol.mjs';

const PACKAGES = fileURLToPath(new URL('./fixtures/packages/', import.meta.url));
const REACT_LIB = 'synthetic-react-lib';
const WC_LIB = '@synthetic/wc-lib';
const PLAIN = 'synthetic-plain';
const KNOWN = [REACT_LIB, WC_LIB, PLAIN];

const tempDirs = [];

afterEach(async () => {
  await Promise.all(tempDirs.splice(0).map((dir) => rm(dir, { recursive: true, force: true })));
});

const makeProject = async () => {
  const dir = await mkdtemp(path.join(os.tmpdir(), 'uigen-install-many-'));
  tempDirs.push(dir);
  return dir;
};

const sourcesOf = (dir) => path.join(dir, '.uigen', 'sources');

const specsOf = (args) => args.filter((arg) => KNOWN.some((name) => arg.startsWith(`${name}@`)));

const splitSpec = (spec) => {
  const at = spec.lastIndexOf('@');
  return { name: spec.slice(0, at), version: spec.slice(at + 1) };
};

const fakeNpm = ({ failWith } = {}) => {
  const calls = [];
  const runNpm = async (args, options) => {
    calls.push({ args: [...args], options });
    if (failWith !== undefined) {
      return failWith;
    }
    const prefix = args[args.indexOf('--prefix') + 1];
    for (const spec of specsOf(args)) {
      const { name } = splitSpec(spec);
      const destination = path.join(prefix, 'node_modules', ...name.split('/'));
      await mkdir(path.dirname(destination), { recursive: true });
      await cp(path.join(PACKAGES, ...name.split('/')), destination, { recursive: true });
    }
    return { code: 0, stdout: '', stderr: '' };
  };
  return { runNpm, calls };
};

const published = async (name) => JSON.parse(await readFile(path.join(PACKAGES, ...name.split('/'), 'package.json'), 'utf8')).version;

describe('install of several packages', () => {
  // covers: INST-4
  it('runs npm once with every name@version, without lifecycle scripts, and reports each package exactly', async () => {
    const dir = await makeProject();
    const { runNpm, calls } = fakeNpm();
    const packages = [
      { name: REACT_LIB, version: '2.4.1' },
      { name: WC_LIB, version: 'latest' },
      { name: PLAIN, version: '3.0.0' },
    ];

    const result = await install({ dir, packages }, { runNpm });

    assert.equal(calls.length, 1);
    const [{ args, options }] = calls;
    assert.equal(args[0], 'install');
    for (const flag of ['--ignore-scripts', '--save-exact']) {
      assert.ok(args.includes(flag), `missing ${flag}`);
    }
    assert.equal(path.resolve(args[args.indexOf('--prefix') + 1]), path.resolve(sourcesOf(dir)));
    assert.deepEqual(specsOf(args).sort(), [`${REACT_LIB}@2.4.1`, `${WC_LIB}@latest`, `${PLAIN}@3.0.0`].sort());
    assert.equal(options.env.npm_config_ignore_scripts, 'true');
    assert.equal(await published(WC_LIB), '1.3.0');
    assert.deepEqual(result, {
      packages: [
        { package: REACT_LIB, version: '2.4.1', license: 'MIT', framework: 'react' },
        { package: WC_LIB, version: '1.3.0', license: 'Apache-2.0', framework: 'web-components' },
        { package: PLAIN, version: '3.0.0', license: null, framework: 'unknown' },
      ],
    });
  });

  // covers: INST-4
  it('still answers the single-package request with the single-package result', async () => {
    const dir = await makeProject();
    const { runNpm, calls } = fakeNpm();

    const result = await install({ dir, package: REACT_LIB, version: '2.4.1' }, { runNpm });

    assert.equal(calls.length, 1);
    assert.equal(calls[0].args.at(-1), `${REACT_LIB}@2.4.1`);
    assert.deepEqual(result, { package: REACT_LIB, version: '2.4.1', license: 'MIT', framework: 'react' });
  });

  // covers: INST-5
  it('turns one npm failure into one install error that names every package of the request', async () => {
    const dir = await makeProject();
    const stderr = ['npm error code E404', `npm error 404 Not Found - GET https://registry.npmjs.org/${PLAIN}`, ''].join('\n');
    const { runNpm, calls } = fakeNpm({ failWith: { code: 1, stdout: '', stderr } });
    const packages = [
      { name: REACT_LIB, version: '2.4.1' },
      { name: PLAIN, version: '3.0.0' },
    ];

    await assert.rejects(install({ dir, packages }, { runNpm }), (error) => {
      assert.ok(error instanceof ServiceError, `expected a ServiceError, got ${String(error)}`);
      assert.equal(error.kind, 'install');
      assert.ok(error.message.includes(REACT_LIB), error.message);
      assert.ok(error.message.includes(PLAIN), error.message);
      assert.ok(error.message.includes(`404 Not Found - GET https://registry.npmjs.org/${PLAIN}`), error.message);
      return true;
    });

    assert.equal(calls.length, 1);
  });
});
