import { describe, expect, it } from 'vitest';
import { clampCodeHeight, codeHeightAfterKey, maxCodeHeight } from '../../../../../src/modules/editor/domain/code/code-panel-height.js';

describe('code panel height', () => {
  it('keeps the maximum between 160 and 600 px', () => {
    expect(maxCodeHeight(0)).toBe(160);
    expect(maxCodeHeight(500)).toBe(320);
    expect(maxCodeHeight(5000)).toBe(600);
  });

  it('clamps and rounds a height between 120 px and the maximum', () => {
    expect(clampCodeHeight(50, 400)).toBe(120);
    expect(clampCodeHeight(900, 400)).toBe(400);
    expect(clampCodeHeight(200.4, 400)).toBe(200);
  });

  it('moves by 16 px with the arrows, jumps with Home and End, ignores other keys', () => {
    expect(codeHeightAfterKey('ArrowUp', 272, 400)).toBe(288);
    expect(codeHeightAfterKey('ArrowDown', 272, 400)).toBe(256);
    expect(codeHeightAfterKey('Home', 272, 400)).toBe(120);
    expect(codeHeightAfterKey('End', 272, 400)).toBe(400);
    expect(codeHeightAfterKey('a', 272, 400)).toBeNull();
  });
});
