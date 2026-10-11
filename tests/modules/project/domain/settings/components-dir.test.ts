import { describe, expect, it } from 'vitest';
import { isValidComponentsDir } from '../../../../../src/modules/project/domain/settings/components-dir';

describe('isValidComponentsDir', () => {
  // covers: FCT-0003
  it('accepts the default components directory', () => {
    expect(isValidComponentsDir('src/components')).toBe(true);
  });

  // covers: FCT-0006
  it.each(['components', 'lib/ui/components', 'src/my-components', 'src/.hidden', 'src/v1.2'])(
    'accepts the relative slash-separated path %j',
    (value) => {
      expect(isValidComponentsDir(value)).toBe(true);
    },
  );

  // covers: FCT-0006
  it('accepts a single dot as the project root', () => {
    expect(isValidComponentsDir('.')).toBe(true);
  });

  // covers: FCT-0006
  it('rejects an empty path', () => {
    expect(isValidComponentsDir('')).toBe(false);
  });

  // covers: FCT-0006
  it.each(['/src/components', '/', '//server/share'])('rejects the absolute path %j', (value) => {
    expect(isValidComponentsDir(value)).toBe(false);
  });

  // covers: FCT-0006
  it.each(['C:/projects/kit/src', 'c:src', 'C:', 'D:/'])('rejects the drive-letter path %j', (value) => {
    expect(isValidComponentsDir(value)).toBe(false);
  });

  // covers: FCT-0006
  it.each(['src\\components', '\\src', 'C:\\src\\components', '\\\\server\\share'])(
    'rejects the backslash path %j',
    (value) => {
      expect(isValidComponentsDir(value)).toBe(false);
    },
  );

  // covers: FCT-0006
  it.each(['src//components', 'src/', 'src/components/'])('rejects the path with an empty segment %j', (value) => {
    expect(isValidComponentsDir(value)).toBe(false);
  });

  // covers: FCT-0006
  it.each(['./src', 'src/./components', 'src/.', './'])('rejects the path with a dot segment %j', (value) => {
    expect(isValidComponentsDir(value)).toBe(false);
  });

  // covers: FCT-0006
  it.each(['..', '../components', 'src/../components', 'src/..', '../../outside'])(
    'rejects the path that climbs out with %j',
    (value) => {
      expect(isValidComponentsDir(value)).toBe(false);
    },
  );
});
