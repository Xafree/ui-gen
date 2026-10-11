import { describe, expect, it } from 'vitest';
import {
  DEFAULT_SCREEN_SIZE,
  formatPixels,
  screenSizeById,
} from '../../../../../src/modules/editor/domain/viewport/screen-size.js';

describe('screen sizes of the frame', () => {
  // covers: FCT-0033
  it('gives 375 px to Mobile, 768 px to Tablette and 1 024 px to Ordinateur, the default', () => {
    expect(screenSizeById('mobile')).toEqual({ id: 'mobile', label: 'Mobile', width: 375 });
    expect(screenSizeById('tablette')).toEqual({ id: 'tablette', label: 'Tablette', width: 768 });
    expect(screenSizeById('ordinateur')).toEqual({ id: 'ordinateur', label: 'Ordinateur', width: 1024 });
    expect(DEFAULT_SCREEN_SIZE).toBe('ordinateur');
  });

  // covers: FCT-0033
  it('writes the width with a French thousands separator and a no-break space before px', () => {
    expect(formatPixels(375).replaceAll(/\s/g, ' ')).toBe('375 px');
    expect(formatPixels(1024).replaceAll(/\s/g, ' ')).toBe('1 024 px');
    expect(formatPixels(1024)).toContain(' px');
  });
});
