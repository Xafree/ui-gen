import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { missingFamiliesOf, textGeometryVerdict } from '../parity-text.mjs';

const origin = { x: 20, y: 20 };

const snapshotText = () => ({
  type: 'text',
  text: 'Bouton',
  box: { x: 36, y: 29, width: 46.5, height: 19 },
  lineBox: { x: 36, y: 29.5, width: 46.5, height: 20 },
  style: { fontFamily: 'Roboto, sans-serif', fontSize: '14px', fontWeight: '400', platformFont: 'Roboto' },
});

const layer = (overrides = {}) => ({
  type: 'TEXT',
  fontFamily: 'Roboto',
  fontStyle: 'Regular',
  fontSize: 14,
  textAutoResize: 'widthAndHeight',
  absoluteBounds: { x: 16, y: 10, width: 47, height: 20 },
  ...overrides,
});

const engine = (missingFamilies = []) => ({ missingFamilies });

describe('the width UIGEN measured', () => {
  it('is compared with the width of the line box and reported next to it', () => {
    const result = textGeometryVerdict([snapshotText()], [layer()], origin, engine());

    assert.equal(result.verdict, 'OK');
    assert.deepEqual(result.matches[0].widths, { chromium: 46.5, uigen: 47 });
  });

  it('fails beyond 1 px', () => {
    const wide = layer({ absoluteBounds: { x: 16, y: 10, width: 48, height: 20 } });
    const result = textGeometryVerdict([snapshotText()], [wide], origin, engine());

    assert.equal(result.verdict, 'KO');
    assert.ok(Math.abs(result.maxDwidth - 1.5) < 1e-9);
  });

  it('is refused when the layer was not sized to its text by UIGEN', () => {
    const fixed = layer({ textAutoResize: 'none' });
    const result = textGeometryVerdict([snapshotText()], [fixed], origin, engine());

    assert.equal(result.verdict, 'KO');
    assert.equal(result.measuredOk, false);
    assert.match(result.reason, /largeur non mesurée/);
  });
});

describe('the font of the layer', () => {
  it('must be installed in UIGEN', () => {
    const result = textGeometryVerdict([snapshotText()], [layer()], origin, engine(['roboto']));

    assert.equal(result.verdict, 'KO');
    assert.equal(result.installedOk, false);
    assert.match(result.reason, /police absente : Roboto/);
  });

  it('is not reported missing when UIGEN lists others only', () => {
    const result = textGeometryVerdict([snapshotText()], [layer()], origin, engine(['Segoe UI']));

    assert.equal(result.installedOk, true);
    assert.equal(result.reason, '');
  });
});

describe('missingFamiliesOf', () => {
  it('reads the families text.missingFonts listed', () => {
    const steps = [
      { command: 'frame.create', result: { id: 2 } },
      { command: 'text.missingFonts', result: { families: [{ family: 'Inter', layers: 1 }, { family: 'Roboto', layers: 2 }] } },
    ];

    assert.deepEqual(missingFamiliesOf(steps), ['Inter', 'Roboto']);
  });

  it('refuses to guess when the command did not answer', () => {
    assert.throws(() => missingFamiliesOf([{ command: 'text.missingFonts', ok: false, error: 'boom' }]), /text\.missingFonts/);
    assert.throws(() => missingFamiliesOf([]), /text\.missingFonts/);
  });
});
