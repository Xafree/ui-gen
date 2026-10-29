import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { collectTexts, textGeometryVerdict, weightOfFontStyle } from '../parity-text.mjs';

const origin = { x: 20, y: 20 };

const snapshotText = (overrides = {}) => ({
  type: 'text',
  text: 'Bouton',
  box: { x: 36, y: 29, width: 46.5, height: 19 },
  lineBox: { x: 36, y: 29.5, width: 46.5, height: 20 },
  style: { fontFamily: 'system-ui, "Segoe UI", sans-serif', fontSize: '14px', fontWeight: '500', platformFont: 'Segoe UI Semibold' },
  ...overrides,
});

const layer = (overrides = {}) => ({
  type: 'TEXT',
  characters: 'Bouton',
  fontFamily: 'Segoe UI',
  fontStyle: 'Semi Bold',
  fontSize: 14,
  absoluteBounds: { x: 16, y: 10, width: 46, height: 20 },
  ...overrides,
});

describe('weightOfFontStyle', () => {
  it('reads the weight a style name stands for', () => {
    const cases = [
      ['Regular', 400],
      ['Medium', 500],
      ['Semi Bold', 600],
      ['Bold Italic', 700],
      ['Extra Light', 200],
      ['Black', 900],
    ];
    for (const [style, weight] of cases) {
      assert.equal(weightOfFontStyle(style), weight, style);
    }
  });
});

describe('collectTexts', () => {
  it('lists the text nodes in document order', () => {
    const tree = { type: 'element', children: [snapshotText({ text: 'a' }), { type: 'element', children: [snapshotText({ text: 'b' })] }] };
    assert.deepEqual(
      collectTexts(tree).map((node) => node.text),
      ['a', 'b'],
    );
  });
});

describe('textGeometryVerdict', () => {
  it('accepts a text whose face matches and whose box is within 1 px', () => {
    const result = textGeometryVerdict([snapshotText()], [layer()], origin);

    assert.equal(result.verdict, 'OK');
    assert.equal(result.familyOk && result.sizeOk && result.weightOk, true);
    assert.ok(Math.abs(result.maxDx) < 1e-9);
    assert.ok(Math.abs(result.maxDy - 0.5) < 1e-9);
    assert.ok(Math.abs(Math.abs(result.maxDwidth) - 0.5) < 1e-9);
  });

  it('accepts a deviation of exactly 1 px and refuses more', () => {
    const exactly = textGeometryVerdict([snapshotText()], [layer({ absoluteBounds: { x: 17, y: 10, width: 46.5, height: 20 } })], origin);
    const beyond = textGeometryVerdict([snapshotText()], [layer({ absoluteBounds: { x: 17.2, y: 10, width: 46.5, height: 20 } })], origin);

    assert.equal(exactly.verdict, 'OK');
    assert.equal(beyond.verdict, 'KO');
    assert.ok(Math.abs(beyond.maxDx - 1.2) < 1e-9);
  });

  it('measures the width and the vertical position against the line box', () => {
    const wide = textGeometryVerdict([snapshotText()], [layer({ absoluteBounds: { x: 16, y: 10, width: 48, height: 20 } })], origin);
    const low = textGeometryVerdict([snapshotText()], [layer({ absoluteBounds: { x: 16, y: 12, width: 46.5, height: 20 } })], origin);

    assert.equal(wide.verdict, 'KO');
    assert.ok(Math.abs(wide.maxDwidth - 1.5) < 1e-9);
    assert.equal(low.verdict, 'KO');
    assert.ok(Math.abs(low.maxDy - 2.5) < 1e-9);
  });

  it('falls back to the text box when there is no line box', () => {
    const text = snapshotText({ lineBox: undefined });
    const result = textGeometryVerdict([text], [layer({ absoluteBounds: { x: 16, y: 9, width: 46.5, height: 19 } })], origin);

    assert.equal(result.verdict, 'OK');
  });

  it('refuses another family, size or weight, and names which one', () => {
    const family = textGeometryVerdict([snapshotText()], [layer({ fontFamily: 'Arial', fontStyle: 'Regular' })], origin);
    const size = textGeometryVerdict([snapshotText()], [layer({ fontSize: 16 })], origin);
    const weight = textGeometryVerdict([snapshotText()], [layer({ fontStyle: 'Regular' })], origin);

    assert.equal(family.verdict, 'KO');
    assert.equal(family.familyOk, false);
    assert.equal(size.verdict, 'KO');
    assert.equal(size.sizeOk, false);
    assert.equal(size.familyOk, true);
    assert.equal(weight.verdict, 'KO');
    assert.equal(weight.weightOk, false);
    assert.equal(weight.familyOk, true);
  });

  it('reads a platform font named with its style as the layer family plus style', () => {
    const plain = snapshotText({ style: { fontSize: '14px', fontWeight: '400', platformFont: 'Roboto Condensed' } });
    const result = textGeometryVerdict([plain], [layer({ fontFamily: 'Roboto Condensed', fontStyle: 'Regular' })], origin);

    assert.equal(result.familyOk, true);
    assert.equal(result.weightOk, true);
  });

  it('judges the weight on the face Chromium drew, not on the requested CSS weight', () => {
    const regularFace = snapshotText({
      style: { fontSize: '14px', fontWeight: '500', platformFont: 'Roboto Condensed', platformFontPostScript: 'RobotoCondensed-Regular' },
    });
    const drawnRegular = textGeometryVerdict([regularFace], [layer({ fontFamily: 'Roboto Condensed', fontStyle: 'Regular' })], origin);
    const drawnMedium = textGeometryVerdict([regularFace], [layer({ fontFamily: 'Roboto Condensed', fontStyle: 'Medium' })], origin);

    assert.equal(drawnRegular.weightOk, true);
    assert.equal(drawnMedium.weightOk, false);
  });

  it('falls back to the first family of the stack when Chromium named no platform font', () => {
    const stack = snapshotText({ style: { fontFamily: '"Inter", sans-serif', fontSize: '14px', fontWeight: 'bold' } });
    const result = textGeometryVerdict([stack], [layer({ fontFamily: 'Inter', fontStyle: 'Bold' })], origin);

    assert.equal(result.verdict, 'OK');
  });

  it('refuses a different number of text layers', () => {
    const result = textGeometryVerdict([snapshotText(), snapshotText()], [layer()], origin);

    assert.equal(result.verdict, 'KO');
    assert.match(result.reason, /2 textes.*1 calque/);
  });

  it('is vacuously fine without any text', () => {
    assert.equal(textGeometryVerdict([], [], origin).verdict, 'OK');
  });
});
