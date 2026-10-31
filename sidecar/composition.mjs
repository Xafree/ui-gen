import { readFile, stat } from 'node:fs/promises';
import path from 'node:path';
import { ServiceError } from './protocol.mjs';

const SHIPPED_DIR = path.join(import.meta.dirname, 'compositions');
const MAX_DEPTH = 12;
const MAX_NODES = 256;
const MAX_TEXT_CHARS = 1024;
const MAX_JSON_CHARS = 64 * 1024;
const MAX_FILE_BYTES = 1024 * 1024;
const TYPE_PATH = /^[A-Z][A-Za-z0-9_]*(\.[A-Z][A-Za-z0-9_]*)*$/;
const REFUSED_PROPS = ['dangerouslySetInnerHTML', 'ref', 'key'];

const isPlainObject = (value) => typeof value === 'object' && value !== null && !Array.isArray(value);

function checkNode(node, depth, counter, refuse) {
  if (depth > MAX_DEPTH) {
    throw refuse(`it is deeper than ${MAX_DEPTH} levels`);
  }
  counter.nodes += 1;
  if (counter.nodes > MAX_NODES) {
    throw refuse(`it has more than ${MAX_NODES} nodes`);
  }
  if (typeof node === 'string') {
    if (depth === 1 || node.length > MAX_TEXT_CHARS) {
      throw refuse(`a text must be a child of at most ${MAX_TEXT_CHARS} characters`);
    }
    return;
  }
  if (!isPlainObject(node) || typeof node.type !== 'string' || !TYPE_PATH.test(node.type)) {
    throw refuse('every node must be {type, props?, children?} with "type" an export path such as "Select.Trigger"');
  }
  const unknown = Object.keys(node).find((key) => !['type', 'props', 'children'].includes(key));
  if (unknown !== undefined) {
    throw refuse(`"${node.type}" has an unknown key "${unknown}"`);
  }
  if (node.props !== undefined && !isPlainObject(node.props)) {
    throw refuse(`the props of "${node.type}" must be a JSON object`);
  }
  const refused = REFUSED_PROPS.find((name) => Object.hasOwn(node.props ?? {}, name));
  if (refused !== undefined) {
    throw refuse(`the prop "${refused}" of "${node.type}" is not allowed`);
  }
  if (node.children !== undefined && !Array.isArray(node.children)) {
    throw refuse(`the children of "${node.type}" must be an array`);
  }
  for (const child of node.children ?? []) {
    checkNode(child, depth + 1, counter, refuse);
  }
}

const slugOf = (packageName) => packageName.replace(/^@/, '').replace('/', '-');

async function readFromFile(file, component) {
  let info;
  try {
    info = await stat(file);
  } catch (error) {
    if (error.code === 'ENOENT' || error.code === 'ENOTDIR') {
      return undefined;
    }
    throw new ServiceError('invalidRequest', `${path.basename(file)} cannot be read: ${error.code}`);
  }
  if (!info.isFile() || info.size > MAX_FILE_BYTES) {
    throw new ServiceError('invalidRequest', `${path.basename(file)} must be a file of at most ${MAX_FILE_BYTES} bytes`);
  }
  let parsed;
  try {
    parsed = JSON.parse(await readFile(file, 'utf8'));
  } catch (error) {
    throw new ServiceError('invalidRequest', `${path.basename(file)} is not valid JSON: ${error.message}`);
  }
  if (!isPlainObject(parsed)) {
    throw new ServiceError('invalidRequest', `${path.basename(file)} must map component names to compositions`);
  }
  return Object.hasOwn(parsed, component) ? checkComposition(parsed[component], component, path.basename(file)) : undefined;
}

export async function compositionOf(dir, packageName, component, shippedDir = SHIPPED_DIR) {
  const name = `${slugOf(packageName)}.json`;
  return (await readFromFile(path.join(dir, '.uigen', 'compositions', name), component)) ?? readFromFile(path.join(shippedDir, name), component);
}

export function checkComposition(value, component, origin = '"composition"') {
  const refuse = (reason) => new ServiceError('invalidRequest', `${origin} for "${component}" is refused: ${reason}`);
  checkNode(value, 1, { nodes: 0 }, refuse);
  if (JSON.stringify(value).length > MAX_JSON_CHARS) {
    throw refuse(`it is larger than ${MAX_JSON_CHARS} characters`);
  }
  if (value.type !== component) {
    throw refuse(`its root must be "${component}", not "${value.type}"`);
  }
  return value;
}
