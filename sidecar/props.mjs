import { readFileSync, realpathSync, statSync } from 'node:fs';
import { readFile } from 'node:fs/promises';
import path from 'node:path';
import { Worker } from 'node:worker_threads';
import { insideRoot } from './installed-package.mjs';

const MEBIBYTE = 1024 * 1024;
const MAX_PROPS = 64;
const MAX_VALUES = 64;
const MAX_NOTE_LENGTH = 300;
const MAX_MANIFEST_BYTES = MEBIBYTE;
const WORKER_URL = new URL('./props-worker.mjs', import.meta.url);
const WORKER_MEMORY_MB = 512;
const WORKER_START_GRACE_MS = 1_000;
const EXCLUDED_NAMES = new Set(['children', 'className', 'style', 'ref', 'key']);
const HANDLER_NAME = /^on[A-Z]/;
const COMPONENT_NAME = /^[A-Z]/;
const STRING_LITERAL = /^(?:'([^']*)'|"([^"]*)")$/;

class BoundError extends Error {}

const isObject = (value) => typeof value === 'object' && value !== null && !Array.isArray(value);

const posix = (file) => file.split(path.sep).join('/');

const groupThousands = (count) => String(count).replace(/\B(?=(\d{3})+(?!\d))/g, ' ');

const describeBytes = (bytes) => (bytes >= MEBIBYTE && bytes % MEBIBYTE === 0 ? `${bytes / MEBIBYTE} MiB` : `${groupThousands(bytes)} bytes`);

const describeFiles = (count) => `${groupThousands(count)} file${count === 1 ? '' : 's'}`;

const clip = (text) => (text.length > MAX_NOTE_LENGTH ? `${text.slice(0, MAX_NOTE_LENGTH)}…` : text);

let typescript;
const loadTypescript = async () => {
  typescript ??= (await import('typescript')).default;
  return typescript;
};

const libraryCache = new Map();

export function startMeter({ budgetMs, maxFiles, maxBytes }) {
  const deadline = Date.now() + budgetMs;
  let files = 0;
  let bytes = 0;
  const tick = () => {
    if (Date.now() > deadline) {
      throw new BoundError(`reading the type declarations took more than ${budgetMs / 1000} s`);
    }
  };
  const count = (size) => {
    tick();
    files += 1;
    bytes += size;
    if (files > maxFiles) {
      throw new BoundError(`the type declarations span more than ${describeFiles(maxFiles)}`);
    }
    if (bytes > maxBytes) {
      throw new BoundError(`the type declarations exceed ${describeBytes(maxBytes)}`);
    }
  };
  return { tick, count };
}

const isWithin = (root, target) => target === root || target.startsWith(`${root}${path.sep}`);

const nodeFileSystem = { realpath: realpathSync.native, stat: statSync, readFile: readFileSync };

function locate(realRoots, file, fileSystem) {
  try {
    const real = fileSystem.realpath(file);
    return realRoots.some((root) => isWithin(root, real)) ? { real, stats: fileSystem.stat(real) } : null;
  } catch {
    return null;
  }
}

export function confinedReader(roots, meter, fileSystem = nodeFileSystem) {
  const realRoots = roots.map((root) => fileSystem.realpath(root));
  const lexicalRoots = roots.map(posix);
  const allowedLexically = (file) => lexicalRoots.some((root) => file === root || file.startsWith(`${root}/`));
  const locateIf = (file, kind) => {
    const found = allowedLexically(file) ? locate(realRoots, file, fileSystem) : null;
    return found !== null && found.stats[kind]() ? found : null;
  };
  const read = (file) => {
    const found = locateIf(file, 'isFile');
    if (found === null) {
      return undefined;
    }
    meter.count(found.stats.size);
    return fileSystem.readFile(found.real, 'utf8');
  };
  const fileExists = (file) => locateIf(file, 'isFile') !== null;
  const directoryExists = (directory) => locateIf(directory, 'isDirectory') !== null;
  return { read, fileExists, directoryExists };
}

function createHost(ts, reader, libDir, currentDirectory) {
  const compilerOptions = { target: ts.ScriptTarget.ES2022 };
  return {
    getSourceFile(fileName, languageVersion) {
      const isLibrary = posix(fileName).startsWith(`${posix(libDir)}/`);
      const text = reader.read(fileName);
      if (text === undefined) {
        return undefined;
      }
      if (isLibrary) {
        const cached = libraryCache.get(fileName);
        if (cached !== undefined && cached.text === text) {
          return cached.source;
        }
        const source = ts.createSourceFile(fileName, text, languageVersion, true);
        libraryCache.set(fileName, { text, source });
        return source;
      }
      return ts.createSourceFile(fileName, text, languageVersion ?? compilerOptions.target, true);
    },
    getDefaultLibFileName: () => posix(path.join(libDir, 'lib.es5.d.ts')),
    getDefaultLibLocation: () => posix(libDir),
    writeFile: () => undefined,
    getCurrentDirectory: () => currentDirectory,
    getCanonicalFileName: (fileName) => (process.platform === 'win32' ? fileName.toLowerCase() : fileName),
    useCaseSensitiveFileNames: () => process.platform !== 'win32',
    getNewLine: () => '\n',
    fileExists: reader.fileExists,
    readFile: (fileName) => reader.read(fileName),
    directoryExists: reader.directoryExists,
    getDirectories: () => [],
  };
}

const COMPILER_OPTIONS = (ts) => ({
  noEmit: true,
  strict: true,
  skipLibCheck: true,
  target: ts.ScriptTarget.ES2022,
  module: ts.ModuleKind.ESNext,
  moduleResolution: ts.ModuleResolutionKind.Bundler,
  lib: ['lib.es5.d.ts', 'lib.dom.d.ts'],
  types: [],
});

function typesEntry(manifest) {
  const fromExports = (value) => {
    if (typeof value === 'string') {
      return value.endsWith('.d.ts') ? value : null;
    }
    if (!isObject(value)) {
      return null;
    }
    if (typeof value.types === 'string') {
      return value.types;
    }
    for (const nested of Object.values(value)) {
      const found = fromExports(nested);
      if (found !== null) {
        return found;
      }
    }
    return null;
  };
  const declared = [manifest.types, manifest.typings].find((value) => typeof value === 'string');
  if (declared !== undefined) {
    return declared;
  }
  const rootExport = isObject(manifest.exports) && '.' in manifest.exports ? manifest.exports['.'] : manifest.exports;
  const exported = fromExports(rootExport);
  if (exported !== null) {
    return exported;
  }
  return typeof manifest.main === 'string' ? manifest.main.replace(/\.[cm]?js$/, '.d.ts') : null;
}

function packageFile(root, file) {
  try {
    const found = locate([realpathSync.native(root)], file, nodeFileSystem);
    return found !== null && found.stats.isFile() ? found : null;
  } catch {
    return null;
  }
}

async function readManifest(root) {
  const found = packageFile(root, path.join(root, 'package.json'));
  if (found === null || found.stats.size > MAX_MANIFEST_BYTES) {
    return {};
  }
  try {
    const manifest = JSON.parse(await readFile(found.real, 'utf8'));
    return isObject(manifest) ? manifest : {};
  } catch {
    return {};
  }
}

function findEntry(manifest, root) {
  const candidates = [typesEntry(manifest), 'index.d.ts'].filter((value) => typeof value === 'string');
  for (const candidate of candidates) {
    const file = insideRoot(root, candidate);
    if (file !== null) {
      try {
        if (statSync(file).isFile()) {
          return posix(file);
        }
      } catch {
        continue;
      }
    }
  }
  return null;
}

const propName = (name) => !EXCLUDED_NAMES.has(name) && !HANDLER_NAME.test(name);

function classifyText(text) {
  const parts = text.split('|').map((part) => part.trim()).filter((part) => part !== 'undefined' && part !== 'null');
  if (parts.length === 0) {
    return { type: 'other', values: null };
  }
  const literals = parts.map((part) => STRING_LITERAL.exec(part)).map((match) => (match === null ? null : (match[1] ?? match[2])));
  if (literals.every((literal) => literal !== null)) {
    return { type: 'enum', values: [...new Set(literals)].slice(0, MAX_VALUES) };
  }
  if (parts.length === 1 && ['string', 'boolean', 'number'].includes(parts[0])) {
    return { type: parts[0], values: null };
  }
  return { type: 'other', values: null };
}

async function readAttributes(installed, meter) {
  const result = new Map();
  const manifest = await readManifest(installed.root);
  if (typeof manifest.customElements !== 'string' || installed.tags.size === 0) {
    return result;
  }
  const file = insideRoot(installed.root, manifest.customElements);
  if (file === null) {
    return result;
  }
  const found = packageFile(installed.root, file);
  if (found === null) {
    return result;
  }
  meter.count(found.stats.size);
  let customElements;
  try {
    customElements = JSON.parse(await readFile(found.real, 'utf8'));
  } catch (error) {
    if (error instanceof BoundError) {
      throw error;
    }
    return result;
  }
  const modules = isObject(customElements) && Array.isArray(customElements.modules) ? customElements.modules : [];
  for (const declaration of modules.filter(isObject).flatMap((module) => (Array.isArray(module.declarations) ? module.declarations : []))) {
    if (isObject(declaration) && typeof declaration.tagName === 'string' && installed.tags.has(declaration.tagName)) {
      result.set(declaration.tagName, attributesOf(declaration));
    }
  }
  return result;
}

function attributesOf(declaration) {
  const fields = new Map((Array.isArray(declaration.members) ? declaration.members : []).filter(isObject).map((member) => [member.name, member]));
  const attributes = Array.isArray(declaration.attributes) ? declaration.attributes : [];
  const props = [];
  for (const attribute of attributes.filter(isObject)) {
    if (typeof attribute.name !== 'string' || !propName(attribute.name)) {
      continue;
    }
    const field = fields.get(attribute.fieldName ?? attribute.name);
    const typeText = [attribute.type?.text, field?.type?.text].find((text) => typeof text === 'string');
    if (typeText === undefined || props.some((prop) => prop.name === attribute.name)) {
      continue;
    }
    props.push({ name: attribute.name, ...classifyText(typeText) });
  }
  return props.slice(0, MAX_PROPS);
}

function createClassifier(ts, checker) {
  const declaredLiterals = (node, depth = 0) => {
    if (depth > 8) {
      return null;
    }
    if (ts.isParenthesizedTypeNode(node)) {
      return declaredLiterals(node.type, depth + 1);
    }
    if (ts.isLiteralTypeNode(node)) {
      return ts.isStringLiteral(node.literal) ? [node.literal.text] : [];
    }
    if (ts.isUnionTypeNode(node)) {
      const parts = node.types.map((member) => declaredLiterals(member, depth + 1));
      return parts.every((part) => part !== null) ? parts.flat() : null;
    }
    if (ts.isTypeReferenceNode(node) && node.typeArguments === undefined) {
      const symbol = checker.getSymbolAtLocation(node.typeName);
      const alias = symbol?.declarations?.find(ts.isTypeAliasDeclaration);
      return alias === undefined ? null : declaredLiterals(alias.type, depth + 1);
    }
    if (node.kind === ts.SyntaxKind.UndefinedKeyword || node.kind === ts.SyntaxKind.NullKeyword) {
      return [];
    }
    return null;
  };

  const orderedValues = (symbol, literals) => {
    const node = symbol.declarations?.find((declaration) => ts.isPropertySignature(declaration) || ts.isPropertyDeclaration(declaration))?.type;
    const declared = node === undefined ? null : declaredLiterals(node);
    const sameSet = declared !== null && declared.length === literals.length && literals.every((value) => declared.includes(value));
    return sameSet ? declared : literals;
  };

  return (symbol) => {
    const type = checker.getNonNullableType(checker.getTypeOfSymbol(symbol));
    if (type.getCallSignatures().length > 0) {
      return null;
    }
    const flags = ts.TypeFlags;
    if (type.flags & flags.BooleanLike) {
      return { type: 'boolean', values: null };
    }
    if (type.flags & flags.String) {
      return { type: 'string', values: null };
    }
    if (type.flags & flags.NumberLike) {
      return { type: 'number', values: null };
    }
    const members = type.isUnion() ? type.types : [type];
    if (members.every((member) => member.flags & flags.StringLiteral)) {
      const literals = [...new Set(members.map((member) => member.value))];
      return { type: 'enum', values: orderedValues(symbol, literals).slice(0, MAX_VALUES) };
    }
    return { type: 'other', values: null };
  };
}

function isOwnDeclaration(declaration, rootPrefix) {
  const file = posix(declaration.getSourceFile().fileName);
  const own = process.platform === 'win32' ? file.toLowerCase().startsWith(rootPrefix.toLowerCase()) : file.startsWith(rootPrefix);
  return own && !file.slice(rootPrefix.length).includes('node_modules/');
}

function componentProps(ts, checker, classify, rootPrefix, exported) {
  const symbol = exported.flags & ts.SymbolFlags.Alias ? checker.getAliasedSymbol(exported) : exported;
  const type = checker.getTypeOfSymbol(symbol);
  const signature = [...type.getCallSignatures(), ...type.getConstructSignatures()].find((candidate) => candidate.getParameters().length > 0);
  const parameter = signature?.getParameters()[0];
  if (parameter === undefined) {
    return null;
  }
  const propsType = checker.getNonNullableType(checker.getTypeOfSymbol(parameter));
  const isObjectLike = propsType.flags & (ts.TypeFlags.Object | ts.TypeFlags.Intersection);
  if (!isObjectLike || propsType.getCallSignatures().length > 0 || checker.isArrayType(propsType)) {
    return null;
  }
  const props = [];
  for (const property of checker.getPropertiesOfType(propsType)) {
    const own = (property.declarations ?? []).some((declaration) => isOwnDeclaration(declaration, rootPrefix));
    if (!own || !propName(property.name)) {
      continue;
    }
    const classified = classify(property);
    if (classified !== null) {
      props.push({ name: property.name, ...classified });
    }
    if (props.length === MAX_PROPS) {
      break;
    }
  }
  return props;
}

function readReactProps(ts, installed, entry, meter) {
  const rootPrefix = `${posix(installed.root)}/`;
  const modulesRoot = path.join(installed.sources, 'node_modules');
  const libDir = path.dirname(ts.sys.getExecutingFilePath());
  const reader = confinedReader([modulesRoot, libDir], meter);
  const host = createHost(ts, reader, libDir, posix(installed.root));
  const program = ts.createProgram({ rootNames: [entry], options: COMPILER_OPTIONS(ts), host });
  const checker = program.getTypeChecker();
  const source = program.getSourceFile(entry);
  const moduleSymbol = source === undefined ? undefined : checker.getSymbolAtLocation(source);
  const components = new Map();
  if (moduleSymbol === undefined) {
    return components;
  }
  const classify = createClassifier(ts, checker);
  for (const exported of checker.getExportsOfModule(moduleSymbol).filter((symbol) => COMPONENT_NAME.test(symbol.name))) {
    meter.tick();
    const props = componentProps(ts, checker, classify, rootPrefix, exported);
    if (props !== null) {
      components.set(exported.name, props);
    }
  }
  return components;
}

export async function readPropsInline(installed, limits) {
  const meter = startMeter(limits);
  const notes = [];
  let components = new Map();
  try {
    components = await readAttributes(installed, meter);
  } catch (error) {
    if (!(error instanceof BoundError)) {
      throw error;
    }
    notes.push(clip(`${error.message}: props not read`));
    return { components: new Map(), notes };
  }
  const entry = findEntry(await readManifest(installed.root), installed.root);
  if (entry === null) {
    notes.push(`${installed.name} ships no type declarations: props not read`);
    return { components, notes };
  }
  try {
    const ts = await loadTypescript();
    for (const [name, props] of readReactProps(ts, installed, entry, meter)) {
      components.set(name, props);
    }
  } catch (error) {
    const reason = error instanceof BoundError ? error.message : `the type declarations cannot be read (${error instanceof Error ? error.message : 'unknown error'})`;
    notes.push(clip(`${reason}: props not read`));
  }
  return { components, notes };
}

const notRead = (reason) => ({ components: new Map(), notes: [clip(`${reason}: props not read`)] });

const failureNote = (error) =>
  error?.code === 'ERR_WORKER_OUT_OF_MEMORY'
    ? 'reading the type declarations used too much memory'
    : `the type declarations cannot be read (${error instanceof Error ? error.message : 'unknown error'})`;

export function readProps(installed, limits, { workerUrl = WORKER_URL, memoryMb = WORKER_MEMORY_MB } = {}) {
  return new Promise((resolve) => {
    const worker = new Worker(workerUrl, { workerData: { installed, limits }, resourceLimits: { maxOldGenerationSizeMb: memoryMb } });
    let timer;
    const finish = (outcome) => {
      clearTimeout(timer);
      worker.removeAllListeners();
      worker.on('error', () => undefined);
      void worker.terminate();
      resolve(outcome);
    };
    timer = setTimeout(
      () => finish(notRead(`reading the type declarations took more than ${limits.budgetMs / 1000} s`)),
      limits.budgetMs + WORKER_START_GRACE_MS,
    );
    worker.once('message', ({ components, notes }) => finish({ components: new Map(components), notes }));
    worker.once('error', (error) => finish(notRead(failureNote(error))));
    worker.once('exit', () => finish(notRead('the type declarations reader stopped without an answer')));
  });
}
