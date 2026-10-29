export const GEOMETRY_TOLERANCE_PX = 1;
const EPSILON = 1e-9;
const REGULAR = 400;

const WEIGHT_NAMES = [
  ['thin', 100],
  ['extralight', 200],
  ['ultralight', 200],
  ['light', 300],
  ['medium', 500],
  ['semibold', 600],
  ['demibold', 600],
  ['extrabold', 800],
  ['ultrabold', 800],
  ['black', 900],
  ['bold', 700],
  ['heavy', 900],
];
const STYLE_WORDS = /^(?:thin|extra|ultra|semi|demi|light|regular|normal|medium|bold|black|heavy|italic|oblique)*$/;

const squash = (name) => name.replaceAll(/[^\p{L}\p{N}]/gu, '').toLowerCase();

export const collectTexts = (node) => (node.type === 'text' ? [node] : (node.children ?? []).flatMap(collectTexts));

const namedWeight = (name) => WEIGHT_NAMES.find(([word]) => squash(name).includes(word))?.[1];

export const weightOfFontStyle = (fontStyle) => namedWeight(fontStyle) ?? REGULAR;

function cssWeight(value) {
  const text = String(value ?? 'normal').trim().toLowerCase();
  if (text === 'normal') return REGULAR;
  if (text === 'bold') return 700;
  const weight = Number.parseFloat(text);
  return Number.isFinite(weight) ? weight : Number.NaN;
}

function weightChromiumUsed(style) {
  const face = style.platformFontPostScript?.split('-').at(-1);
  if (face !== undefined && /^(?:regular|italic)$/i.test(face)) return REGULAR;
  return (face === undefined ? undefined : namedWeight(face)) ?? namedWeight(style.platformFont ?? '') ?? cssWeight(style.fontWeight);
}

const firstFamily = (stack) => stack.split(',')[0]?.trim().replaceAll(/^["']|["']$/g, '') ?? '';

function familyMatches(style, layer) {
  const actual = squash(layer.fontFamily ?? '');
  if (actual === '') return false;
  const expected = squash(style.platformFont ?? firstFamily(style.fontFamily ?? ''));
  if (expected === actual) return true;
  return style.platformFont !== undefined && expected.startsWith(actual) && STYLE_WORDS.test(expected.slice(actual.length));
}

function compareText(text, layer, origin) {
  const expected = text.lineBox ?? text.box;
  const actual = layer.absoluteBounds;
  return {
    familyOk: familyMatches(text.style, layer),
    sizeOk: Math.abs(Number.parseFloat(text.style.fontSize) - layer.fontSize) < EPSILON,
    weightOk: weightChromiumUsed(text.style) === weightOfFontStyle(layer.fontStyle ?? ''),
    dx: actual.x - (expected.x - origin.x),
    dy: actual.y - (expected.y - origin.y),
    dwidth: actual.width - expected.width,
  };
}

const maxAbs = (values) => values.reduce((worst, value) => (Math.abs(value) > Math.abs(worst) ? value : worst), 0);

export function textGeometryVerdict(texts, layers, origin) {
  if (texts.length !== layers.length) {
    const reason = `${texts.length} textes capturés, ${layers.length} calque(s) texte importé(s)`;
    return { verdict: 'KO', reason, familyOk: false, sizeOk: false, weightOk: false, maxDx: 0, maxDy: 0, maxDwidth: 0, matches: [] };
  }
  const matches = texts.map((text, index) => compareText(text, layers[index], origin));
  const maxDx = maxAbs(matches.map((match) => match.dx));
  const maxDy = maxAbs(matches.map((match) => match.dy));
  const maxDwidth = maxAbs(matches.map((match) => match.dwidth));
  const familyOk = matches.every((match) => match.familyOk);
  const sizeOk = matches.every((match) => match.sizeOk);
  const weightOk = matches.every((match) => match.weightOk);
  const withinTolerance = [maxDx, maxDy, maxDwidth].every((value) => Math.abs(value) <= GEOMETRY_TOLERANCE_PX + EPSILON);
  const verdict = familyOk && sizeOk && weightOk && withinTolerance ? 'OK' : 'KO';
  return { verdict, reason: '', familyOk, sizeOk, weightOk, maxDx, maxDy, maxDwidth, matches };
}
