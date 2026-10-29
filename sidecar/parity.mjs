import { spawnSync } from 'node:child_process';
import { existsSync } from 'node:fs';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import pixelmatch from 'pixelmatch';
import { PNG } from 'pngjs';
import { GEOMETRY_TOLERANCE_PX, collectTexts, textGeometryVerdict } from './parity-text.mjs';

const here = import.meta.dirname;
const repoRoot = resolve(here, '..');
const outDir = resolve(here, 'out', 'parity');
const cli = resolve(repoRoot, 'target', 'debug', process.platform === 'win32' ? 'uigen-cli.exe' : 'uigen-cli');
const LIBS = ['mui', 'chakra', 'shadcn'];
const STATES = ['default', 'hover'];
const FRAME_NAME = 'parity-frame';
const PIXELMATCH_THRESHOLD = 0.1;
const SHAPES_MAX_PERCENT = 1;
const SCANNED_IDS_MARGIN = 4;
const DIFF_PIXEL = [255, 0, 0];

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

const countNodes = (node) => 1 + (node.children ?? []).reduce((total, child) => total + countNodes(child), 0);

function nodeQueries(snapshot) {
  const lastId = countNodes(snapshot.root) + SCANNED_IDS_MARGIN;
  return Array.from({ length: lastId }, (_, index) => ({ command: 'node.get', params: { id: index + 1 } }));
}

function parseSteps(output) {
  return output
    .split('\n')
    .filter(Boolean)
    .map((line) => JSON.parse(line));
}

const textLayersOf = (steps) =>
  steps
    .filter((step) => step.command === 'node.get' && step.result?.type === 'TEXT')
    .map((step) => step.result)
    .sort((a, b) => a.id - b.id);

function textMask(snapshot, width, height) {
  const mask = new Uint8Array(width * height);
  const originX = Math.floor(snapshot.root.box.x);
  const originY = Math.floor(snapshot.root.box.y);
  for (const text of collectTexts(snapshot.root)) {
    for (const box of [text.box, text.lineBox].filter(Boolean)) {
      const left = Math.max(0, Math.floor(box.x) - originX);
      const top = Math.max(0, Math.floor(box.y) - originY);
      const right = Math.min(width, Math.ceil(box.x + box.width) - originX);
      const bottom = Math.min(height, Math.ceil(box.y + box.height) - originY);
      for (let y = top; y < bottom; y += 1) {
        mask.fill(1, y * width + left, y * width + Math.max(left, right));
      }
    }
  }
  return mask;
}

function splitDifferences(diff, mask) {
  let shapes = 0;
  let text = 0;
  let textPixels = 0;
  for (let index = 0; index < mask.length; index += 1) {
    textPixels += mask[index];
    const at = index * 4;
    if (diff.data[at] === DIFF_PIXEL[0] && diff.data[at + 1] === DIFF_PIXEL[1] && diff.data[at + 2] === DIFF_PIXEL[2]) {
      if (mask[index] === 1) {
        text += 1;
      } else {
        shapes += 1;
      }
    }
  }
  return { shapes, text, textPixels };
}

const percentOf = (count, total) => (total > 0 ? (count / total) * 100 : 0);

function compare(capture, render, snapshot) {
  const width = Math.min(capture.width, render.width);
  const height = Math.min(capture.height, render.height);
  const left = crop(capture, width, height);
  const right = crop(render, width, height);
  const diff = new PNG({ width, height });
  const differing = pixelmatch(left.data, right.data, diff.data, width, height, { threshold: PIXELMATCH_THRESHOLD, includeAA: false });
  const sizeMismatch = capture.width !== render.width || capture.height !== render.height;
  const unionPixels = Math.max(capture.width, render.width) * Math.max(capture.height, render.height);
  const extraPixels = unionPixels - width * height;
  const split = splitDifferences(diff, textMask(snapshot, width, height));
  return { diff, width, height, differing, sizeMismatch, extraPixels, unionPixels, split };
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
    ...nodeQueries(snapshot),
  ];
  const scriptPath = resolve(outDir, `${name}.script.json`);
  const documentPath = resolve(outDir, `${name}.figcraft`);
  const renderPath = resolve(outDir, `${name}-render.png`);
  await writeFile(scriptPath, JSON.stringify(script));
  const runOutput = run(cli, ['run', scriptPath, '--out', documentPath]);
  const steps = parseSteps(runOutput);
  const warnings = steps.find((step) => step.command === 'uigen.importDom')?.result?.warnings ?? [];
  const geometry = textGeometryVerdict(collectTexts(snapshot.root), textLayersOf(steps), snapshot.root.box);
  run(cli, ['export', documentPath, '-o', renderPath, '--node', FRAME_NAME, '--scale', '1']);

  const render = await readPng(renderPath);
  const result = compare(capture, render, snapshot);
  await writeFile(resolve(outDir, `${name}-diff.png`), PNG.sync.write(result.diff));
  const { split } = result;
  const shapesDiffering = split.shapes + result.extraPixels;
  const shapesPercent = percentOf(shapesDiffering, result.unionPixels - split.textPixels);
  const textPercent = percentOf(split.text, split.textPixels);
  return {
    geometry,
    name,
    captureSize: `${capture.width}x${capture.height}`,
    renderSize: `${render.width}x${render.height}`,
    shapesDiffering,
    textDiffering: split.text,
    textPixels: split.textPixels,
    shapesPercent,
    textPercent,
    sizeMismatch: result.sizeMismatch,
    warnings,
    shapesVerdict: shapesPercent < SHAPES_MAX_PERCENT ? 'OK' : 'KO',
    textVerdict: geometry.verdict,
  };
}

const yesNo = (value) => (value ? 'oui' : 'non');
const signed = (value) => value.toFixed(2);

function table(rows) {
  const lines = [
    '| cas | taille capture | taille rendu | pixels formes | % formes | verdict formes | verdict texte (géométrie) | dx max (px) | dy max (px) | dlargeur max (px) | police égale | taille égale | graisse égale | pixels texte (info) | zone texte (px) | % texte (info) |',
    '|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|',
    ...rows.map((row) => {
      const { geometry } = row;
      return `| ${row.name} | ${row.captureSize} | ${row.renderSize} | ${row.shapesDiffering} | ${row.shapesPercent.toFixed(2)} % | ${row.shapesVerdict} | ${row.textVerdict} | ${signed(geometry.maxDx)} | ${signed(geometry.maxDy)} | ${signed(geometry.maxDwidth)} | ${yesNo(geometry.familyOk)} | ${yesNo(geometry.sizeOk)} | ${yesNo(geometry.weightOk)} | ${row.textDiffering} | ${row.textPixels} | ${row.textPercent.toFixed(2)} % |`;
    }),
  ];
  const warnings = rows.flatMap((row) => row.warnings.map((warning) => `- ${row.name} : ${warning}`));
  const reasons = rows.filter((row) => row.geometry.reason !== '').map((row) => `- ${row.name} : ${row.geometry.reason}`);
  const note = `Seuil de pixelmatch : ${PIXELMATCH_THRESHOLD}, includeAA: false (pixels anticrénelés ignorés). Formes (hors boîtes de texte, lineBox ∪ box) : < ${SHAPES_MAX_PERCENT} % = OK. Texte : même police (famille), même taille, même graisse que Chromium, et position (x, y) et largeur de la couche à ${GEOMETRY_TOLERANCE_PX} px près de la boîte de ligne (lineBox, sinon box) = OK ; l'écart pixel du texte est donné à titre d'information et ne fait pas échouer (les glyphes diffèrent entre DirectWrite et le moteur de texte d'UIGEN).`;
  const section = (title, items) => `## ${title}\n\n${items.length > 0 ? items.join('\n') : 'aucun'}`;
  const parts = [lines.join('\n'), note, section('Avertissements de uigen.importDom', warnings), section('Textes non appariés', reasons)];
  return `${parts.join('\n\n')}\n`;
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
  if (rows.some((row) => row.shapesVerdict !== 'OK' || row.textVerdict !== 'OK')) {
    process.exitCode = 1;
  }
} catch (error) {
  console.error(`erreur : ${error.message}`);
  process.exit(1);
}
