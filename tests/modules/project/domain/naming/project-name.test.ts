import { describe, expect, it } from 'vitest';
import {
  deriveProjectName,
  validateProjectName,
  type ProjectNameError,
} from '../../../../../src/modules/project/domain/naming/project-name';

describe('validateProjectName', () => {
  it.each(['Kit Horizon', 'Atlas', 'Kit 2', '42'])('accepts %j', (value) => {
    expect(validateProjectName(value)).toBeNull();
  });

  it('refuses an empty name as empty', () => {
    const expected: ProjectNameError = 'empty';
    expect(validateProjectName('')).toBe(expected);
  });

  it.each(['---', '!?', '. _ -'])('refuses %j, which has no letter nor digit', (value) => {
    expect(validateProjectName(value)).toBe('no-alphanumeric');
  });
});

describe('deriveProjectName', () => {
  it('takes the scope of a scoped package', () => {
    expect(deriveProjectName('@atlas/components')).toBe('Atlas');
  });

  it('capitalises each dashed word of the scope', () => {
    expect(deriveProjectName('@kit-horizon/ui')).toBe('Kit Horizon');
  });

  it('takes the name of an unscoped package', () => {
    expect(deriveProjectName('my-lib')).toBe('My Lib');
  });

  it('splits words on dashes, underscores and dots', () => {
    expect(deriveProjectName('my_design.kit-core')).toBe('My Design Kit Core');
  });

  it('capitalises a single word', () => {
    expect(deriveProjectName('lit')).toBe('Lit');
  });
});
