import { afterEach, describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { cp, lstat, mkdir, readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { install, uninstall } from '../install.mjs';
import { npmCommand } from '../run-npm.mjs';
import { ServiceError } from '../protocol.mjs';
import { makeTempDir } from './helpers/review-project.mjs';

const REACT_LIB = 'synthetic-react-lib';
const PACKAGES = fileURLToPath(new URL('./fixtures/packages/', import.meta.url));

const cleanups = [];

afterEach(async () => {
  await Promise.all(cleanups.splice(0).map((cleanup) => cleanup()));
});

const makeProject = async () => {
  const project = await makeTempDir('uigen-npmrc-');
  cleanups.push(project.remove);
  return project.dir;
};

const optionAfter = (args, flag) => args[args.indexOf(flag) + 1];

const recordingNpm = () => {
  const calls = [];
  const runNpm = async (args, options) => {
    const prefix = optionAfter(args, '--prefix');
    const userConfig = optionAfter(args, '--userconfig');
    calls.push({ args: [...args], options, userConfigSize: (await readFile(userConfig)).length });
    if (args[0] === 'install') {
      const destination = path.join(prefix, 'node_modules', REACT_LIB);
      await mkdir(path.dirname(destination), { recursive: true });
      await cp(path.join(PACKAGES, REACT_LIB), destination, { recursive: true });
    }
    return { code: 0, stdout: '', stderr: '' };
  };
  return { runNpm, calls };
};

describe('npm configuration of the project', () => {
  it('runs install with an empty user and global npm configuration', async () => {
    const dir = await makeProject();
    const { runNpm, calls } = recordingNpm();

    await install({ dir, package: REACT_LIB, version: '2.4.1' }, { runNpm });

    const [{ args, options, userConfigSize }] = calls;
    const userConfig = optionAfter(args, '--userconfig');
    assert.equal(optionAfter(args, '--globalconfig'), userConfig);
    assert.ok(path.isAbsolute(userConfig));
    assert.ok(!path.resolve(userConfig).startsWith(path.resolve(dir)), 'the empty configuration lives outside the project');
    assert.equal(userConfigSize, 0);
    assert.equal(options.env.npm_config_userconfig, userConfig);
    assert.equal(options.env.npm_config_globalconfig, userConfig);
    assert.equal(options.env.npm_config_ignore_scripts, 'true');
    assert.ok(args.includes('--ignore-scripts'));
    assert.equal(args[args.length - 1], `${REACT_LIB}@2.4.1`);
  });

  it('runs uninstall with an empty user and global npm configuration', async () => {
    const dir = await makeProject();
    const { runNpm, calls } = recordingNpm();
    await install({ dir, package: REACT_LIB, version: '2.4.1' }, { runNpm });

    await uninstall({ dir, package: REACT_LIB }, { runNpm });

    const { args, options, userConfigSize } = calls[1];
    assert.equal(args[0], 'uninstall');
    assert.equal(optionAfter(args, '--globalconfig'), optionAfter(args, '--userconfig'));
    assert.equal(userConfigSize, 0);
    assert.equal(options.env.npm_config_userconfig, optionAfter(args, '--userconfig'));
  });

  it('refuses to install when .uigen/sources/.npmrc exists, without running npm', async () => {
    const dir = await makeProject();
    const sources = path.join(dir, '.uigen', 'sources');
    await mkdir(sources, { recursive: true });
    await writeFile(path.join(sources, '.npmrc'), 'ignore-scripts=false\n');
    const { runNpm, calls } = recordingNpm();

    await assert.rejects(install({ dir, package: REACT_LIB, version: '2.4.1' }, { runNpm }), (error) => {
      assert.ok(error instanceof ServiceError);
      assert.equal(error.kind, 'install');
      assert.ok(error.message.includes('.npmrc'), error.message);
      return true;
    });
    assert.equal(calls.length, 0);
    await assert.doesNotReject(lstat(path.join(sources, '.npmrc')));
  });

  it('refuses to uninstall when .uigen/sources/.npmrc exists, without running npm', async () => {
    const dir = await makeProject();
    const { runNpm, calls } = recordingNpm();
    await install({ dir, package: REACT_LIB, version: '2.4.1' }, { runNpm });
    await writeFile(path.join(dir, '.uigen', 'sources', '.npmrc'), 'ignore-scripts=false\n');

    await assert.rejects(uninstall({ dir, package: REACT_LIB }, { runNpm }), (error) => {
      assert.ok(error instanceof ServiceError);
      assert.equal(error.kind, 'uninstall');
      assert.ok(error.message.includes('.npmrc'), error.message);
      return true;
    });
    assert.equal(calls.length, 1);
  });
});

describe('npm command', () => {
  const nodeDir = path.join(path.sep, 'tools', 'node');

  it('runs the npm-cli.js found next to node with node itself', () => {
    const cli = path.join(nodeDir, 'node_modules', 'npm', 'bin', 'npm-cli.js');

    const command = npmCommand(['install'], { execPath: path.join(nodeDir, 'node'), npmExecPath: undefined, exists: (candidate) => candidate === cli });

    assert.deepEqual(command, { command: path.join(nodeDir, 'node'), args: [cli, 'install'] });
  });

  it('gives no command, and never a shell fallback, when npm-cli.js cannot be found', () => {
    const command = npmCommand(['install'], { execPath: path.join(nodeDir, 'node'), npmExecPath: undefined, exists: () => false });

    assert.equal(command, null);
  });
});
