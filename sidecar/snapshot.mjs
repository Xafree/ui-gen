export const LIMITS = Object.freeze({
  maxJsonBytes: 8 * 1024 * 1024,
  maxNodes: 5000,
  maxDepth: 64,
  maxMarkupBytes: 1024 * 1024,
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
  ];
  const TEXT_KEYS = [
    'color', 'fontFamily', 'fontSize', 'fontWeight', 'fontStyle', 'lineHeight', 'letterSpacing',
    'textTransform', 'textAlign',
  ];
  const SKIPPED_TAGS = new Set(['script', 'style', 'noscript', 'template', 'link', 'meta', 'title']);
  const LABEL_MAX = 40;
  const encoder = new TextEncoder();
  let nodeCount = 0;

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
    return { type: 'svg', markup, box: boxOf(svg.getBoundingClientRect()) };
  };

  const textNode = (node) => {
    const text = node.textContent.replace(/\s+/g, ' ').trim();
    if (!text) {
      return undefined;
    }
    const range = document.createRange();
    range.selectNodeContents(node);
    const box = boxOf(range.getBoundingClientRect());
    return { type: 'text', text, box, style: pick(getComputedStyle(node.parentElement), TEXT_KEYS) };
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
    for (const child of element.childNodes) {
      if (child.nodeType === Node.TEXT_NODE) {
        const text = textNode(child);
        if (text) {
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
  const root = elementNode(rootElement, 0);
  if (!root) {
    fail(`snapshot refused: "${rootSelector}" is hidden (display:none or visibility:hidden)`);
  }
  return { root, devicePixelRatio: window.devicePixelRatio, innerWidth: window.innerWidth, innerHeight: window.innerHeight };
}

export async function captureSnapshot(page, rootSelector, source) {
  const walked = await page.evaluate(walkInPage, { rootSelector, limits: LIMITS }).catch((error) => {
    throw new Error(error.message.replace(/^page\.evaluate: (Error: )?/, ''), { cause: error });
  });
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
