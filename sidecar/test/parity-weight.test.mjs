import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { textGeometryVerdict } from '../parity-text.mjs';

const origin = { x: 0, y: 0 };

const snapshotText = (style) => ({
  type: 'text',
  text: 'Bouton',
  box: { x: 0, y: 0, width: 46, height: 20 },
  style: { fontSize: '14px', ...style },
});

const layer = (fontFamily, fontStyle) => ({
  type: 'TEXT',
  fontFamily,
  fontStyle,
  fontSize: 14,
  absoluteBounds: { x: 0, y: 0, width: 46, height: 20 },
});

const weightOk = (style, fontStyle) => textGeometryVerdict([snapshotText(style)], [layer('Arial', fontStyle)], origin).weightOk;

describe('the weight of the face Chromium drew', () => {
  it('reads a PostScript name without a style suffix as the Regular face', () => {
    for (const postScript of ['ArialMT', 'SegoeUI', 'Roboto']) {
      const style = { fontFamily: 'Arial', fontWeight: '500', platformFont: 'Arial', platformFontPostScript: postScript };
      assert.equal(weightOk(style, 'Regular'), true, postScript);
      assert.equal(weightOk(style, 'Medium'), false, postScript);
    }
  });

  it('reads a lone MT suffix as the Regular face', () => {
    const style = { fontFamily: 'Arial', fontWeight: '700', platformFont: 'Arial', platformFontPostScript: 'Arial-MT' };

    assert.equal(weightOk(style, 'Regular'), true);
  });

  it('reads the style a suffix names, MT or not', () => {
    const cases = [
      ['Arial-BoldMT', 'Bold'],
      ['Arial-ItalicMT', 'Italic'],
      ['Arial-BoldItalicMT', 'Bold Italic'],
      ['SegoeUI-Semibold', 'Semi Bold'],
    ];
    for (const [postScript, fontStyle] of cases) {
      const style = { fontFamily: 'Arial', fontWeight: '400', platformFont: 'Arial', platformFontPostScript: postScript };
      assert.equal(weightOk(style, fontStyle), true, postScript);
    }
  });
});
