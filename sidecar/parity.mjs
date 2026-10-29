import { spawnSync } from 'node:child_process';
import { existsSync } from 'node:fs';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import pixelmatch from 'pixelmatch';
import { PNG } from 'pngjs';

const here = import.meta.dirname;
const repoRoot = resolve(here, '..');
const outDir = resolve(here, 'out', 'parity');
const cli = resolve(repoRoot, 'target', 'debug', process.platform === 'win32' ? 'uigen-cli.exe' : 'uigen-cli');
const LIBS = ['mui', 'chakra', 'shadcn'];
const STATES = ['default', 'hover'];
const FRAME_NAME = 'parity-frame';
const PIXELMATCH_THRESHOLD = 0.1;
const VERDICT_MAX_PERCENT = 2;

function run(command, args, options = {}) {
  const result = spawnSync(command, args, { encoding: 'utf8', maxBuffer: 64 * 1024 * 1024, ...options });
  if (result.status !== 0) {
    throw new Error(`${command} ${args.join(' ')} a échoué (code ${result.status}) : ${result.stderr || result.stdout}`);
  }
  return result.stdout;
}

async function readPng(file) {
  return PNG.sync.read(await readFile(file));
}

function crop(png, width, height) {
  const out = new PNG({ width, height });
  PNG.bitblt(png, out, 0, 0, width, height, 0, 0);
  return out;
}

function compare(capture, render) {
  const width = Math.min(capture.width, render.width);
  const height = Math.min(capture.height, render.height);
  const left = crop(capture, width, height);
  const right = crop(render, width, height);
  const diff = new PNG({ width, height });
  const differing = pixelmatch(left.data, right.data, diff.data, width, height, { threshold: PIXELMATCH_THRESHOLD });
  const sizeMismatch = capture.width !== render.width || capture.height !== render.height;
  const unionPixels = Math.max(capture.width, render.width) * Math.max(capture.height, render.height);
  const extraPixels = unionPixels - width * height;
  return { diff, width, height, differing, sizeMismatch, extraPixels, unionPixels };
}

async function measure(lib, state) {
  const name = `${lib}-${state}`;
  run(process.execPath, [resolve(here, 'capture.mjs'), lib, state, outDir]);
  const snapshot = JSON.parse(await readFile(resolve(outDir, `${name}.json`), 'utf8'));
  const capture = await readPng(resolve(outDir, `${name}.png`));

  const script = [
    {
      command: 'frame.create',
      as: 'frame',
      params: { x: 0, y: 0, width: capture.width, height: capture.height, name: FRAME_NAME, props: { fill: '#FFFFFF', clip: true } },
    },
    { command: 'uigen.importDom', as: 'imported', params: { snapshot, parent: { $ref: 'frame' }, x: 0, y: 0 } },
  ];
  const scriptPath = resolve(outDir, `${name}.script.json`);
  const documentPath = resolve(outDir, `${name}.figcraft`);
  const renderPath = resolve(outDir, `${name}-render.png`);
  await writeFile(scriptPath, JSON.stringify(script));
  const runOutput = run(cli, ['run', scriptPath, '--out', documentPath]);
  const importStep = runOutput
    .split('\n')
    .filter(Boolean)
    .map((line) => JSON.parse(line))
    .find((step) => step.command === 'uigen.importDom');
  const warnings = importStep?.result?.warnings ?? [];
  run(cli, ['export', documentPath, '-o', renderPath, '--node', FRAME_NAME, '--scale', '1']);

  const render = await readPng(renderPath);
  const result = compare(capture, render);
  await writeFile(resolve(outDir, `${name}-diff.png`), PNG.sync.write(result.diff));
  const differing = result.differing + result.extraPixels;
  const percent = (differing / result.unionPixels) * 100;
  const intersectionPercent = (result.differing / (result.width * result.height)) * 100;
  return {
    name,
    captureSize: `${capture.width}x${capture.height}`,
    renderSize: `${render.width}x${render.height}`,
    differing,
    intersectionDiffering: result.differing,
    percent,
    intersectionPercent,
    sizeMismatch: result.sizeMismatch,
    warnings,
    verdict: percent < VERDICT_MAX_PERCENT ? 'OK' : 'KO',
  };
}

function table(rows) {
  const lines = [
    '| cas | taille capture | taille rendu | pixels différents | % d\'écart | % sur l\'intersection | verdict |',
    '|---|---|---|---|---|---|---|',
    ...rows.map(
      (row) =>
        `| ${row.name} | ${row.captureSize} | ${row.renderSize} | ${row.differing} | ${row.percent.toFixed(2)} % | ${row.intersectionPercent.toFixed(2)} % | ${row.verdict} |`,
    ),
  ];
  const warnings = rows.flatMap((row) => row.warnings.map((warning) => `- ${row.name} : ${warning}`));
  return `${lines.join('\n')}\n\nSeuil de pixelmatch : ${PIXELMATCH_THRESHOLD} (antialiasing compté). Verdict : < ${VERDICT_MAX_PERCENT} % = OK.\n\n## Avertissements de uigen.importDom\n\n${warnings.length > 0 ? warnings.join('\n') : 'aucun'}\n`;
}

try {
  if (!existsSync(cli)) {
    throw new Error(`CLI absent : ${cli} ; lancer cargo build -p figcraft-cli`);
  }
  await mkdir(outDir, { recursive: true });
  const rows = [];
  for (const lib of LIBS) {
    for (const state of STATES) {
      rows.push(await measure(lib, state));
    }
  }
  const report = table(rows);
  await writeFile(resolve(outDir, 'parity.md'), report);
  console.log(report);
  if (rows.some((row) => row.verdict !== 'OK')) {
    process.exitCode = 1;
  }
} catch (error) {
  console.error(`erreur : ${error.message}`);
  process.exit(1);
}
