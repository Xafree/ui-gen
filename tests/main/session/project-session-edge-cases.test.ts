import { describe, expect, it } from 'vitest';
import { ProjectSession } from '../../../src/main/session/project-session';

const DOCUMENTS = String.raw`C:\Users\emma\Documents`;

describe('ProjectSession edge cases', () => {
  // covers: FCT-0007
  it('recognizes the pending library whatever the case of the path', () => {
    const session = new ProjectSession(DOCUMENTS);
    session.setPendingAdoption(String.raw`E:\projets\Legacy-Kit`);

    expect(session.isPendingAdoption(String.raw`e:\PROJETS\legacy-kit`)).toBe(true);
  });

  // covers: FCT-0007
  it('does not recognize a sibling whose name only starts like the pending library', () => {
    const session = new ProjectSession(DOCUMENTS);
    session.setPendingAdoption(String.raw`E:\projets\legacy-kit`);

    expect(session.isPendingAdoption(String.raw`E:\projets\legacy-kit-evil`)).toBe(false);
    expect(session.isPendingAdoption(String.raw`E:\projets\legacy-ki`)).toBe(false);
  });

  // covers: FCT-0003
  it('does not grant a folder inside a granted location', () => {
    const session = new ProjectSession(DOCUMENTS);
    session.grantLocation(String.raw`E:\projets`);

    expect(session.isLocationGranted(String.raw`E:\projets\kit-horizon`)).toBe(false);
  });
});
