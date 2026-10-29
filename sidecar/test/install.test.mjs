import { afterEach, describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { cp, mkdir, mkdtemp, readFile, rm, stat, writeFile } from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { install, uninstall } from '../install.mjs';
import { ServiceError } from '../protocol.mjs';

const PACKAGES = fileURLToPath(new URL('./fixtures/packages/', import.meta.url));
const REACT_LIB = 'synthetic-react-lib';

const tempDirs = [];

afterEach(async () => {
  await Promise.all(tempDirs.splice(0).map((dir) => rm(dir, { recursive: true, force: true })));
});

const makeProject = async () => {
  const dir = await mkdtemp(path.join(os.tmpdir(), 'uigen-install-'));
  tempDirs.push(dir);
  return dir;
};

const sourcesOf = (dir) => path.join(dir, '.uigen', 'sources');

const splitSpec = (spec) => {
  const at = spec.lastIndexOf('@');
  return { name: spec.slice(0, at), version: spec.slice(at + 1) };
};

const exists = async (file) => {
  try {
    await stat(file);
    return true;
  } catch {
    return false;
  }
};

const fakeNpm = ({ failWith } = {}) => {
  const calls = [];
  const runNpm = async (args, options) => {
    const prefix = args[args.indexOf('--prefix') + 1];
    const snapshot = { args: [...args], options, sourcesManifestExisted: await exists(path.join(prefix, 'package.json')) };
    calls.push(snapshot);
    if (failWith !== undefined) {
      return failWith;
    }
    const target = args[args.length - 1];
    if (args[0] === 'install') {
      const { name, version } = splitSpec(target);
      const source = path.join(PACKAGES, ...name.split('/'));
      const published = JSON.parse(await readFile(path.join(source, 'package.json'), 'utf8')).version;
      if (version !== 'latest' && version !== published) {
        return { code: 1, stdout: '', stderr: `npm error notarget No matching version found for ${target}.\n` };
      }
      const destination = path.join(prefix, 'node_modules', ...name.split('/'));
      await mkdir(path.dirname(destination), { recursive: true });
      await cp(source, destination, { recursive: true });
    } else if (args[0] === 'uninstall') {
      await rm(path.join(prefix, 'node_modules', ...target.split('/')), { recursive: true, force: true });
    }
    return { code: 0, stdout: '', stderr: '' };
  };
  return { runNpm, calls };
};

const rejectsWith = (promise, kind) =>
  assert.rejects(promise, (error) => {
    assert.ok(error instanceof ServiceError, `expected a ServiceError, got ${String(error)}`);
    assert.equal(error.kind, kind);
    assert.equal(typeof error.message, 'string');
    assert.notEqual(error.message.trim(), '');
    return true;
  });

describe('install', () => {
  // covers: E2-2.1
  it('runs npm without lifecycle scripts, in the project sources folder, and reports the installed package', async () => {
    const dir = await makeProject();
    const { runNpm, calls } = fakeNpm();

    const result = await install({ dir, package: REACT_LIB, version: '2.4.1' }, { runNpm });

    assert.equal(calls.length, 1);
    const [{ args, options }] = calls;
    assert.equal(args[0], 'install');
    for (const flag of ['--ignore-scripts', '--save-exact', '--no-audit', '--no-fund']) {
      assert.ok(args.includes(flag), `missing ${flag}`);
    }
    const prefixAt = args.indexOf('--prefix');
    assert.notEqual(prefixAt, -1);
    assert.equal(path.resolve(args[prefixAt + 1]), path.resolve(sourcesOf(dir)));
    assert.equal(args[args.length - 1], `${REACT_LIB}@2.4.1`);
    assert.equal(options.env.npm_config_ignore_scripts, 'true');
    assert.deepEqual(result, { package: REACT_LIB, version: '2.4.1', license: 'MIT', framework: 'react' });
  });

  // covers: E2-2.1
  it('reports the exact installed version when asked for latest', async () => {
    const dir = await makeProject();
    const { runNpm, calls } = fakeNpm();

    const result = await install({ dir, package: REACT_LIB, version: 'latest' }, { runNpm });

    assert.equal(calls[0].args[calls[0].args.length - 1], `${REACT_LIB}@latest`);
    assert.equal(result.version, '2.4.1');
  });

  // covers: E2-2.1
  it('creates .uigen/sources/package.json before npm runs when it is missing', async () => {
    const dir = await makeProject();
    const { runNpm, calls } = fakeNpm();

    await install({ dir, package: REACT_LIB, version: '2.4.1' }, { runNpm });

    assert.equal(calls[0].sourcesManifestExisted, true);
    const manifest = JSON.parse(await readFile(path.join(sourcesOf(dir), 'package.json'), 'utf8'));
    assert.equal(typeof manifest, 'object');
    assert.notEqual(manifest, null);
  });

  // covers: E2-2.1
  it('keeps an existing .uigen/sources/package.json as it is', async () => {
    const dir = await makeProject();
    const existing = `${JSON.stringify({ name: 'my-sources', private: true, dependencies: { other: '1.0.0' } }, null, 2)}\n`;
    await mkdir(sourcesOf(dir), { recursive: true });
    await writeFile(path.join(sourcesOf(dir), 'package.json'), existing);
    const { runNpm } = fakeNpm();

    await install({ dir, package: REACT_LIB, version: '2.4.1' }, { runNpm });

    assert.equal(await readFile(path.join(sourcesOf(dir), 'package.json'), 'utf8'), existing);
  });

  // covers: E2-2.1
  it('reports a null license when package.json declares none', async () => {
    const dir = await makeProject();
    const { runNpm } = fakeNpm();

    const result = await install({ dir, package: 'synthetic-plain', version: '3.0.0' }, { runNpm });

    assert.equal(result.license, null);
  });

  // covers: E2-2.2
  it('detects the framework from package.json', async () => {
    const cases = [
      { name: REACT_LIB, version: '2.4.1', framework: 'react' },
      { name: '@synthetic/wc-lib', version: '1.3.0', framework: 'web-components' },
      { name: 'synthetic-elements', version: '0.9.0', framework: 'web-components' },
      { name: 'synthetic-plain', version: '3.0.0', framework: 'unknown' },
    ];
    for (const { name, version, framework } of cases) {
      const dir = await makeProject();
      const { runNpm } = fakeNpm();

      const result = await install({ dir, package: name, version }, { runNpm });

      assert.equal(result.package, name);
      assert.equal(result.framework, framework, name);
    }
  });

  // covers: E2-2.3
  it('refuses an invalid package name without running npm', async () => {
    const dir = await makeProject();
    for (const name of ['../x', 'file:../x', 'git+https://example.com/x.git', '-x', 'Mui', 'x.tgz', '', undefined]) {
      const { runNpm, calls } = fakeNpm();

      await rejectsWith(install({ dir, package: name, version: '1.0.0' }, { runNpm }), 'invalidRequest');

      assert.equal(calls.length, 0, `npm was called for ${String(name)}`);
    }
  });

  // covers: E2-2.3
  it('refuses an invalid version without running npm', async () => {
    const dir = await makeProject();
    for (const version of ['^1.0.0', '~1.2.3', '*', 'next', '1.0', '1.0.0 || 2.0.0', '', undefined]) {
      const { runNpm, calls } = fakeNpm();

      await rejectsWith(install({ dir, package: REACT_LIB, version }, { runNpm }), 'invalidRequest');

      assert.equal(calls.length, 0, `npm was called for ${String(version)}`);
    }
  });

  // covers: E2-2.3
  it('refuses a relative, missing or absent dir without running npm', async () => {
    const existing = await makeProject();
    for (const dir of ['relative/project', path.join(existing, 'missing'), undefined]) {
      const { runNpm, calls } = fakeNpm();

      await rejectsWith(install({ dir, package: REACT_LIB, version: '2.4.1' }, { runNpm }), 'invalidRequest');

      assert.equal(calls.length, 0, `npm was called for dir ${String(dir)}`);
    }
  });

  // covers: E2-2.4
  it('turns an npm failure into an install error citing the package, the version and the last useful npm line', async () => {
    const dir = await makeProject();
    const stderr = [
      'npm warn deprecated left-pad@1.0.0: no longer maintained',
      'npm error code E404',
      `npm error 404 Not Found - GET https://registry.npmjs.org/${REACT_LIB}`,
      '',
      '',
    ].join('\n');
    const { runNpm } = fakeNpm({ failWith: { code: 1, stdout: '', stderr } });

    await assert.rejects(install({ dir, package: REACT_LIB, version: '2.4.1' }, { runNpm }), (error) => {
      assert.ok(error instanceof ServiceError, `expected a ServiceError, got ${String(error)}`);
      assert.equal(error.kind, 'install');
      assert.ok(error.message.includes(REACT_LIB), error.message);
      assert.ok(error.message.includes('2.4.1'), error.message);
      assert.ok(error.message.includes(`404 Not Found - GET https://registry.npmjs.org/${REACT_LIB}`), error.message);
      assert.ok(!error.message.includes('no longer maintained'), error.message);
      return true;
    });
  });

  // covers: E2-2.4
  it('reports a version that npm cannot find as an install error', async () => {
    const dir = await makeProject();
    const { runNpm } = fakeNpm();

    await rejectsWith(install({ dir, package: REACT_LIB, version: '9.9.9' }, { runNpm }), 'install');
  });
});

describe('uninstall', () => {
  const seedInstalled = async (dir, name) => {
    const target = path.join(sourcesOf(dir), 'node_modules', ...name.split('/'));
    await mkdir(target, { recursive: true });
    await cp(path.join(PACKAGES, ...name.split('/')), target, { recursive: true });
  };

  // covers: E2-2.5
  it('runs npm uninstall without lifecycle scripts and leaves .uigen/captures alone', async () => {
    const dir = await makeProject();
    await seedInstalled(dir, REACT_LIB);
    const captures = path.join(dir, '.uigen', 'captures');
    await mkdir(captures, { recursive: true });
    await writeFile(path.join(captures, 'button.png'), 'png bytes');
    const { runNpm, calls } = fakeNpm();

    const result = await uninstall({ dir, package: REACT_LIB }, { runNpm });

    assert.deepEqual(result, { package: REACT_LIB });
    assert.equal(calls.length, 1);
    const { args } = calls[0];
    assert.equal(args[0], 'uninstall');
    assert.ok(args.includes('--ignore-scripts'));
    const prefixAt = args.indexOf('--prefix');
    assert.notEqual(prefixAt, -1);
    assert.equal(path.resolve(args[prefixAt + 1]), path.resolve(sourcesOf(dir)));
    assert.equal(args[args.length - 1], REACT_LIB);
    assert.equal(await exists(path.join(sourcesOf(dir), 'node_modules', REACT_LIB)), false);
    assert.equal(await readFile(path.join(captures, 'button.png'), 'utf8'), 'png bytes');
  });

  // covers: E2-2.5
  it('reports notInstalled for a package that is not in the project sources', async () => {
    const dir = await makeProject();
    const { runNpm } = fakeNpm();

    await rejectsWith(uninstall({ dir, package: REACT_LIB }, { runNpm }), 'notInstalled');
  });

  // covers: E2-2.5
  it('reports notInstalled when other packages are installed but not this one', async () => {
    const dir = await makeProject();
    await seedInstalled(dir, 'synthetic-plain');
    const { runNpm } = fakeNpm();

    await rejectsWith(uninstall({ dir, package: REACT_LIB }, { runNpm }), 'notInstalled');
  });

  // covers: E2-2.3
  it('refuses an invalid package name or dir without running npm', async () => {
    const dir = await makeProject();
    const cases = [
      { dir, package: '../x' },
      { dir, package: '-x' },
      { dir: 'relative/project', package: REACT_LIB },
      { dir: path.join(dir, 'missing'), package: REACT_LIB },
    ];
    for (const params of cases) {
      const { runNpm, calls } = fakeNpm();

      await rejectsWith(uninstall(params, { runNpm }), 'invalidRequest');

      assert.equal(calls.length, 0);
    }
  });
});
