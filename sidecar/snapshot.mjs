export const LIMITS = Object.freeze({
  maxJsonBytes: 8 * 1024 * 1024,
  maxNodes: 5000,
  maxDepth: 64,
  maxMarkupBytes: 1024 * 1024,
  maxClassNameLength: 256,
  maxVars: 64,
  maxAnimations: 16,
  maxAnimationText: 256,
  maxAnimationProperties: 64,
  maxPropertyLength: 128,
});

function walkInPage({ rootSelector, limits }) {
  const ELEMENT_KEYS = [
    'display', 'visibility', 'opacity', 'overflow', 'backgroundColor', 'backgroundImage',
    'borderTopWidth', 'borderRightWidth', 'borderBottomWidth', 'borderLeftWidth',
    'borderTopColor', 'borderRightColor', 'borderBottomColor', 'borderLeftColor',
    'borderTopStyle', 'borderRightStyle', 'borderBottomStyle', 'borderLeftStyle',
    'borderTopLeftRadius', 'borderTopRightRadius', 'borderBottomRightRadius', 'borderBottomLeftRadius',
    'boxShadow', 'outlineWidth', 'outlineColor', 'outlineStyle', 'outlineOffset',
    'color', 'fontFamily', 'fontSize', 'fontWeight', 'fontStyle', 'lineHeight', 'letterSpacing',
    'textTransform', 'textAlign',
    'transitionProperty', 'transitionDuration', 'transitionTimingFunction', 'transitionDelay',
    'flexDirection', 'flexWrap', 'justifyContent', 'alignItems', 'alignContent', 'alignSelf',
    'flexGrow', 'flexShrink', 'flexBasis', 'rowGap', 'columnGap',
    'paddingTop', 'paddingRight', 'paddingBottom', 'paddingLeft',
    'position', 'boxSizing', 'gridTemplateColumns',
  ];
  const SHORTHANDS = new Map([
    ['padding', ['paddingTop', 'paddingRight', 'paddingBottom', 'paddingLeft']],
    ['gap', ['rowGap', 'columnGap']],
    ['border-color', ['borderTopColor', 'borderRightColor', 'borderBottomColor', 'borderLeftColor']],
    ['border-width', ['borderTopWidth', 'borderRightWidth', 'borderBottomWidth', 'borderLeftWidth']],
    ['border-radius', ['borderTopLeftRadius', 'borderTopRightRadius', 'borderBottomRightRadius', 'borderBottomLeftRadius']],
  ]);
  const BARE_VAR = /^var\(\s*(--[A-Za-z0-9_-]{1,128})\s*(?:,[\s\S]*)?\)$/;
  const NO_SPECIFICITY = [0, 0, 0];
  const TEXT_KEYS = [
    'color', 'fontFamily', 'fontSize', 'fontWeight', 'fontStyle', 'lineHeight', 'letterSpacing',
    'textTransform', 'textAlign', 'textDecorationLine',
  ];
  const SKIPPED_TAGS = new Set(['script', 'style', 'noscript', 'template', 'link', 'meta', 'title']);
  const LABEL_MAX = 40;
  const encoder = new TextEncoder();
  const MARK_ATTRIBUTE = 'data-uigen-text-id';
  let nodeCount = 0;
  let markCount = 0;

  const fail = (message) => {
    throw new Error(message);
  };

  const countNode = () => {
    nodeCount += 1;
    if (nodeCount > limits.maxNodes) {
      fail(`snapshot refused: more than ${limits.maxNodes} nodes`);
    }
  };

  const pick = (computed, keys) => {
    const style = {};
    for (const key of keys) {
      style[key] = computed[key];
    }
    return style;
  };

  const kebab = (key) => key.replace(/[A-Z]/g, (letter) => `-${letter.toLowerCase()}`);

  const compareRanks = (left, right) => {
    for (let index = 0; index < left.length; index += 1) {
      if (left[index] !== right[index]) {
        return left[index] - right[index];
      }
    }
    return 0;
  };

  const strongest = (items, rankOf) =>
    items.reduce((best, item) => (best === undefined || compareRanks(rankOf(item), rankOf(best)) > 0 ? item : best), undefined);

  const splitSelectors = (selectors) => {
    const parts = [];
    let depth = 0;
    let start = 0;
    for (let index = 0; index < selectors.length; index += 1) {
      const character = selectors[index];
      if (character === '(' || character === '[') {
        depth += 1;
      } else if (character === ')' || character === ']') {
        depth -= 1;
      } else if (character === ',' && depth === 0) {
        parts.push(selectors.slice(start, index));
        start = index + 1;
      }
    }
    parts.push(selectors.slice(start));
    return parts.map((part) => part.trim()).filter(Boolean);
  };

  const specificityOf = (selector) => {
    const total = [0, 0, 0];
    const count = (slot) => () => {
      total[slot] += 1;
      return ' ';
    };
    selector
      .replace(/\\./g, 'x')
      .replace(/"[^"]*"|'[^']*'/g, '')
      .replace(/:where\((?:[^()]|\([^()]*\))*\)/g, '')
      .replace(/:(?:is|not|has|matches)\(((?:[^()]|\([^()]*\))*)\)/g, (_, args) => {
        const inner = strongest(splitSelectors(args).map(specificityOf), (value) => value) ?? NO_SPECIFICITY;
        inner.forEach((value, slot) => {
          total[slot] += value;
        });
        return ' ';
      })
      .replace(/\[[^\]]*\]/g, count(1))
      .replace(/#[\w-]+/g, count(0))
      .replace(/::[\w-]+|:(?:before|after|first-line|first-letter)\b/g, count(2))
      .replace(/:[\w-]+(?:\([^)]*\))?/g, count(1))
      .replace(/\.[\w-]+/g, count(1))
      .replace(/(^|[\s>+~])[a-zA-Z][\w-]*/g, (_, lead) => {
        total[2] += 1;
        return lead;
      });
    return total;
  };

  const declarationsOf = (declaration) => {
    const found = [];
    const add = (key, name, value) =>
      found.push({ key, value, name: BARE_VAR.exec(value)?.[1], important: declaration.getPropertyPriority(name) === 'important' });
    const longhands = new Set();
    for (const key of ELEMENT_KEYS) {
      const value = declaration.getPropertyValue(kebab(key)).trim();
      if (value) {
        longhands.add(key);
        add(key, kebab(key), value);
      }
    }
    for (const [shorthand, keys] of SHORTHANDS) {
      const value = declaration.getPropertyValue(shorthand).trim();
      if (value.includes('var(')) {
        keys.filter((key) => !longhands.has(key)).forEach((key) => add(key, shorthand, value));
      }
    }
    return found;
  };

  const mediaApplies = (media) => media.length === 0 || matchMedia(media.mediaText).matches;

  const styleRulesOf = (sheet) => {
    let rules;
    try {
      rules = [...sheet.cssRules];
    } catch {
      return [];
    }
    if (sheet.disabled || (sheet.media && !mediaApplies(sheet.media))) {
      return [];
    }
    return rules.flatMap((rule) => {
      if (rule instanceof CSSStyleRule) {
        return [rule];
      }
      if (rule instanceof CSSImportRule) {
        return rule.styleSheet && mediaApplies(rule.media) ? styleRulesOf(rule.styleSheet) : [];
      }
      if (rule instanceof CSSMediaRule && !mediaApplies(rule.media)) {
        return [];
      }
      if (rule instanceof CSSSupportsRule && !CSS.supports(rule.conditionText)) {
        return [];
      }
      return rule.cssRules && !(rule instanceof CSSKeyframesRule) ? styleRulesOf({ cssRules: rule.cssRules }) : [];
    });
  };

  const buildDeclarationIndex = () => {
    const index = new Map();
    [...document.styleSheets, ...document.adoptedStyleSheets].flatMap(styleRulesOf).forEach((rule, order) => {
      const selectors = splitSelectors(rule.selectorText).map((text) => ({ text, specificity: specificityOf(text) }));
      for (const declaration of declarationsOf(rule.style)) {
        const entries = index.get(declaration.key) ?? { all: [], named: [] };
        const entry = { selectors, order, ...declaration };
        entries.all.push(entry);
        if (entry.name !== undefined) {
          entries.named.push(entry);
        }
        index.set(declaration.key, entries);
      }
    });
    return index;
  };

  const declarationIndex = buildDeclarationIndex();

  const matches = (element, selector) => {
    try {
      return element.matches(selector);
    } catch {
      return false;
    }
  };

  const rankOf = (element, entry) => {
    const ranks = entry.selectors
      .filter((selector) => matches(element, selector.text))
      .map((selector) => [entry.important ? 2 : 0, ...selector.specificity, entry.order]);
    return strongest(ranks, (rank) => rank);
  };

  const varsOf = (element) => {
    const inline = declarationsOf(element.style);
    const keys = new Set(inline.filter((declaration) => declaration.name !== undefined).map((declaration) => declaration.key));
    for (const [key, { named }] of declarationIndex) {
      if (named.some((entry) => rankOf(element, entry) !== undefined)) {
        keys.add(key);
      }
    }
    const vars = [];
    for (const key of keys) {
      const candidates = (declarationIndex.get(key)?.all ?? []).map((entry) => ({ rank: rankOf(element, entry), name: entry.name }));
      for (const declaration of inline.filter((other) => other.key === key)) {
        candidates.push({ rank: [declaration.important ? 3 : 1, 0, 0, 0, 0], name: declaration.name });
      }
      const winner = strongest(candidates.filter((candidate) => candidate.rank !== undefined), (candidate) => candidate.rank);
      if (winner?.name !== undefined) {
        vars.push([key, winner.name]);
      }
    }
    return Object.fromEntries(vars.slice(0, limits.maxVars));
  };

  const animatedPropertiesOf = (animation) =>
    [...new Set(animation.effect.getKeyframes().flatMap((keyframe) => Object.keys(keyframe)))]
      .filter((key) => !['offset', 'computedOffset', 'easing', 'composite'].includes(key))
      .map((key) => kebab(key).replace(/^css-/, '').slice(0, limits.maxPropertyLength))
      .slice(0, limits.maxAnimationProperties);

  const animationsOf = (element) =>
    element
      .getAnimations()
      .filter((animation) => (animation instanceof CSSAnimation || animation instanceof CSSTransition) && animation.effect)
      .slice(0, limits.maxAnimations)
      .map((animation) => {
        const timing = animation.effect.getComputedTiming();
        return {
          name: (animation instanceof CSSAnimation ? animation.animationName : animation.transitionProperty).slice(0, limits.maxAnimationText),
          durationMs: Number.isFinite(Number(timing.duration)) ? Number(timing.duration) : 0,
          easing: animation.effect.getTiming().easing.slice(0, limits.maxAnimationText),
          iterations: Number.isFinite(timing.iterations) ? timing.iterations : 'infinite',
          properties: animatedPropertiesOf(animation),
        };
      });

  const boxOf = (rect) => ({
    x: rect.x + window.scrollX,
    y: rect.y + window.scrollY,
    width: rect.width,
    height: rect.height,
  });

  const isHidden = (computed) => computed.display === 'none' || computed.visibility === 'hidden';

  const labelOf = (element) => {
    const aria = element.getAttribute('aria-label');
    if (aria?.trim()) {
      return aria.trim();
    }
    const text = (element.innerText ?? '').replace(/\s+/g, ' ').trim();
    if (!text) {
      return undefined;
    }
    return text.length > LABEL_MAX ? `${text.slice(0, LABEL_MAX)}…` : text;
  };

  const resolveSvgColors = (source) => {
    const clone = source.cloneNode(true);
    const originals = [source, ...source.querySelectorAll('*')];
    const clones = [clone, ...clone.querySelectorAll('*')];
    originals.forEach((original, index) => {
      const target = clones[index];
      const computed = getComputedStyle(original);
      for (const attribute of [...target.attributes]) {
        if (/currentcolor/i.test(attribute.value)) {
          target.setAttribute(attribute.name, attribute.value.replace(/currentcolor/gi, computed.color));
        }
      }
      target.setAttribute('color', computed.color);
      target.setAttribute('fill', computed.fill);
      target.setAttribute('stroke', computed.stroke);
    });
    return clone.outerHTML;
  };

  const svgNode = (svg) => {
    const markup = resolveSvgColors(svg);
    if (encoder.encode(markup).length > limits.maxMarkupBytes) {
      fail(`snapshot refused: svg markup larger than ${limits.maxMarkupBytes} bytes`);
    }
    const node = { type: 'svg', markup, box: boxOf(svg.getBoundingClientRect()) };
    const animations = animationsOf(svg);
    if (animations.length > 0) {
      node.animations = animations;
    }
    return node;
  };

  const normalLineHeight = (parent) => {
    const probe = document.createElement('span');
    probe.style.cssText = 'position:absolute;visibility:hidden;display:inline-block;margin:0;padding:0;border:0;';
    probe.textContent = 'X';
    parent.append(probe);
    const { height } = probe.getBoundingClientRect();
    probe.remove();
    return height;
  };

  const lineHeightOf = (parent, computed) => {
    const px = computed.lineHeight.endsWith('px') ? Number.parseFloat(computed.lineHeight) : Number.NaN;
    return Number.isFinite(px) ? px : normalLineHeight(parent);
  };

  const metricsCanvas = document.createElement('canvas').getContext('2d');

  const fontAscentOf = (computed) => {
    metricsCanvas.font = `${computed.fontStyle} ${computed.fontWeight} ${computed.fontSize} ${computed.fontFamily}`;
    return Math.round(metricsCanvas.measureText('').fontBoundingBoxAscent);
  };

  const isSingleLine = (range) => new Set([...range.getClientRects()].filter((rect) => rect.width > 0).map((rect) => Math.round(rect.top))).size <= 1;

  const markParent = (parent) => {
    if (!parent.hasAttribute(MARK_ATTRIBUTE)) {
      markCount += 1;
      parent.setAttribute(MARK_ATTRIBUTE, String(markCount));
    }
    return parent.getAttribute(MARK_ATTRIBUTE);
  };

  const textNode = (node) => {
    const text = node.textContent.replace(/\s+/g, ' ').trim();
    if (!text) {
      return undefined;
    }
    const range = document.createRange();
    range.selectNodeContents(node);
    const box = boxOf(range.getBoundingClientRect());
    const parent = node.parentElement;
    const computed = getComputedStyle(parent);
    const result = { type: 'text', text, box, style: pick(computed, TEXT_KEYS), markId: markParent(parent), baseline: box.y + fontAscentOf(computed) };
    if (isSingleLine(range)) {
      const height = lineHeightOf(parent, computed);
      if (Number.isFinite(height) && height > 0) {
        result.lineBox = { x: box.x, y: box.y + box.height / 2 - height / 2, width: box.width, height };
      }
    }
    return result;
  };

  const elementNode = (element, depth) => {
    if (depth > limits.maxDepth) {
      fail(`snapshot refused: depth greater than ${limits.maxDepth}`);
    }
    const computed = getComputedStyle(element);
    if (isHidden(computed)) {
      return undefined;
    }
    countNode();
    const tag = element.localName;
    if (tag === 'svg') {
      return svgNode(element);
    }
    const node = {
      type: 'element',
      tag,
      box: boxOf(element.getBoundingClientRect()),
      style: pick(computed, ELEMENT_KEYS),
      children: [],
    };
    const label = labelOf(element);
    if (label) {
      node.label = label;
    }
    const className = element.getAttribute('class');
    if (className) {
      node.className = className.slice(0, limits.maxClassNameLength);
    }
    const vars = varsOf(element);
    if (Object.keys(vars).length > 0) {
      node.vars = vars;
    }
    const animations = animationsOf(element);
    if (animations.length > 0) {
      node.animations = animations;
    }
    for (const child of element.childNodes) {
      if (child.nodeType === Node.TEXT_NODE) {
        const text = textNode(child);
        if (text) {
          if (depth + 1 > limits.maxDepth) {
            fail(`snapshot refused: depth greater than ${limits.maxDepth}`);
          }
          countNode();
          node.children.push(text);
        }
      } else if (child.nodeType === Node.ELEMENT_NODE && !SKIPPED_TAGS.has(child.localName)) {
        const converted = elementNode(child, depth + 1);
        if (converted) {
          node.children.push(converted);
        }
      }
    }
    return node;
  };

  const rootElement = document.querySelector(rootSelector);
  if (!rootElement) {
    fail(`snapshot refused: no element matches "${rootSelector}"`);
  }
  const root = elementNode(rootElement, 1);
  if (!root) {
    fail(`snapshot refused: "${rootSelector}" is hidden (display:none or visibility:hidden)`);
  }
  return JSON.stringify({ root, devicePixelRatio: window.devicePixelRatio, innerWidth: window.innerWidth, innerHeight: window.innerHeight });
}

const MARK_ATTRIBUTE = 'data-uigen-text-id';

const textsOf = (node) => (node.type === 'text' ? [node] : (node.children ?? []).flatMap(textsOf));

async function platformFontOfMarks(page, marks) {
  const fontOfMark = new Map();
  if (marks.length === 0) {
    return fontOfMark;
  }
  const cdp = await page.context().newCDPSession(page);
  try {
    await cdp.send('DOM.enable');
    await cdp.send('CSS.enable');
    const { root } = await cdp.send('DOM.getDocument', { depth: 0 });
    for (const mark of marks) {
      const { nodeId } = await cdp.send('DOM.querySelector', { nodeId: root.nodeId, selector: `[${MARK_ATTRIBUTE}="${mark}"]` });
      if (nodeId) {
        const { fonts } = await cdp.send('CSS.getPlatformFontsForNode', { nodeId });
        const used = fonts.reduce((best, font) => (best === undefined || font.glyphCount > best.glyphCount ? font : best), undefined);
        if (used?.familyName) {
          fontOfMark.set(mark, { family: used.familyName, postScript: used.postScriptName });
        }
      }
    }
  } finally {
    await cdp.detach();
  }
  return fontOfMark;
}

async function recordPlatformFonts(page, root) {
  const texts = textsOf(root);
  const fontOfMark = await platformFontOfMarks(page, [...new Set(texts.map((text) => text.markId))]);
  for (const text of texts) {
    const font = fontOfMark.get(text.markId);
    if (font !== undefined) {
      text.style.platformFont = font.family;
      if (font.postScript) {
        text.style.platformFontPostScript = font.postScript;
      }
    }
    delete text.markId;
  }
}

export async function captureSnapshot(page, rootSelector, source) {
  const walked = JSON.parse(
    await page.evaluate(walkInPage, { rootSelector, limits: LIMITS }).catch((error) => {
      throw new Error(error.message.replace(/^page\.evaluate: (Error: )?/, ''), { cause: error });
    }),
  );
  try {
    await recordPlatformFonts(page, walked.root);
  } finally {
    await page.evaluate((attribute) => {
      for (const element of document.querySelectorAll(`[${attribute}]`)) {
        element.removeAttribute(attribute);
      }
    }, MARK_ATTRIBUTE);
  }
  const size = page.viewportSize() ?? { width: walked.innerWidth, height: walked.innerHeight };
  const snapshot = {
    version: 1,
    ...(source === undefined ? {} : { source }),
    viewport: { width: size.width, height: size.height, deviceScaleFactor: walked.devicePixelRatio },
    root: walked.root,
  };
  const bytes = Buffer.byteLength(JSON.stringify(snapshot));
  if (bytes > LIMITS.maxJsonBytes) {
    throw new Error(`snapshot refused: ${bytes} bytes of JSON, limit is ${LIMITS.maxJsonBytes}`);
  }
  return snapshot;
}
