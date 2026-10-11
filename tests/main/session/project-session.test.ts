import { describe, expect, it } from 'vitest';
import type { OpenedProject } from '../../../src/modules/project/application/open/project-outcomes';
import { ProjectSession } from '../../../src/main/session/project-session';

const DOCUMENTS = 'C:\\Users\\emma\\Documents';
const CHOSEN_LOCATION = 'E:\\projets';
const LEGACY_LIBRARY = 'E:\\projets\\legacy-kit';
const OTHER_LIBRARY = 'E:\\projets\\other-kit';

const kitHorizon: OpenedProject = {
  path: 'E:\\projets\\kit-horizon',
  name: 'Kit Horizon',
  packageName: '@kit-horizon/ui',
  version: '0.1.0',
  settings: { formatVersion: 1, tagPrefix: 'kh-', componentsDir: 'src/components' },
};

const legacyKit: OpenedProject = {
  path: LEGACY_LIBRARY,
  name: 'legacy-kit',
  packageName: '@acme/legacy-kit',
  version: '2.4.1',
  settings: { formatVersion: 1, tagPrefix: 'lk-', componentsDir: 'src/elements' },
};

describe('ProjectSession locations', () => {
  it('keeps the Documents folder as the default location', () => {
    expect(new ProjectSession(DOCUMENTS).defaultLocation).toBe(DOCUMENTS);
  });

  // covers: FCT-0003
  it('grants the default location without any choice', () => {
    expect(new ProjectSession(DOCUMENTS).isLocationGranted(DOCUMENTS)).toBe(true);
  });

  // covers: FCT-0003
  it('grants the default location whatever the case', () => {
    expect(new ProjectSession(DOCUMENTS).isLocationGranted('c:\\USERS\\Emma\\documents')).toBe(true);
  });

  // covers: FCT-0003
  it('does not grant a location that was never chosen', () => {
    const session = new ProjectSession(DOCUMENTS);

    expect(session.isLocationGranted(CHOSEN_LOCATION)).toBe(false);
    expect(session.isLocationGranted('C:\\Windows\\System32')).toBe(false);
  });

  // covers: FCT-0003
  it('grants a location once it has been chosen', () => {
    const session = new ProjectSession(DOCUMENTS);

    session.grantLocation(CHOSEN_LOCATION);

    expect(session.isLocationGranted(CHOSEN_LOCATION)).toBe(true);
    expect(session.isLocationGranted('e:\\PROJETS')).toBe(true);
  });

  // covers: FCT-0003
  it('keeps every chosen location granted', () => {
    const session = new ProjectSession(DOCUMENTS);

    session.grantLocation(CHOSEN_LOCATION);
    session.grantLocation('\\\\serveur\\partage\\projets');

    expect(session.isLocationGranted(CHOSEN_LOCATION)).toBe(true);
    expect(session.isLocationGranted('\\\\serveur\\partage\\projets')).toBe(true);
    expect(session.isLocationGranted(DOCUMENTS)).toBe(true);
  });

  // covers: FCT-0003
  it('does not grant a sibling whose name only starts like a granted location', () => {
    const session = new ProjectSession(DOCUMENTS);

    session.grantLocation(CHOSEN_LOCATION);

    expect(session.isLocationGranted('E:\\projets-evil')).toBe(false);
    expect(session.isLocationGranted('C:\\Users\\emma\\Documents-evil')).toBe(false);
  });

  it('keeps the default location unchanged when another location is granted', () => {
    const session = new ProjectSession(DOCUMENTS);

    session.grantLocation(CHOSEN_LOCATION);

    expect(session.defaultLocation).toBe(DOCUMENTS);
  });
});

describe('ProjectSession pending adoption', () => {
  // covers: FCT-0007
  it('has no pending adoption at start', () => {
    expect(new ProjectSession(DOCUMENTS).isPendingAdoption(LEGACY_LIBRARY)).toBe(false);
  });

  // covers: FCT-0007
  it('remembers the library waiting for the adoption confirmation', () => {
    const session = new ProjectSession(DOCUMENTS);

    session.setPendingAdoption(LEGACY_LIBRARY);

    expect(session.isPendingAdoption(LEGACY_LIBRARY)).toBe(true);
    expect(session.isPendingAdoption(OTHER_LIBRARY)).toBe(false);
  });

  // covers: FCT-0007
  it('only keeps the last library proposed for adoption', () => {
    const session = new ProjectSession(DOCUMENTS);

    session.setPendingAdoption(LEGACY_LIBRARY);
    session.setPendingAdoption(OTHER_LIBRARY);

    expect(session.isPendingAdoption(OTHER_LIBRARY)).toBe(true);
    expect(session.isPendingAdoption(LEGACY_LIBRARY)).toBe(false);
  });

  // covers: FCT-0008
  it('forgets the pending adoption when the confirmation is canceled', () => {
    const session = new ProjectSession(DOCUMENTS);
    session.setPendingAdoption(LEGACY_LIBRARY);

    session.setPendingAdoption(null);

    expect(session.isPendingAdoption(LEGACY_LIBRARY)).toBe(false);
  });

  // covers: FCT-0008
  it('stays without an open project when the adoption is canceled', () => {
    const session = new ProjectSession(DOCUMENTS);
    session.setPendingAdoption(LEGACY_LIBRARY);

    session.setPendingAdoption(null);

    expect(session.currentProject).toBeNull();
  });

  // covers: FCT-0007
  it('clears the pending adoption once the adopted library is opened', () => {
    const session = new ProjectSession(DOCUMENTS);
    session.setPendingAdoption(LEGACY_LIBRARY);

    session.openProject(legacyKit);

    expect(session.isPendingAdoption(LEGACY_LIBRARY)).toBe(false);
    expect(session.currentProject).toEqual(legacyKit);
  });

  // covers: FCT-0007
  it('clears the pending adoption when another project is opened', () => {
    const session = new ProjectSession(DOCUMENTS);
    session.setPendingAdoption(LEGACY_LIBRARY);

    session.openProject(kitHorizon);

    expect(session.isPendingAdoption(LEGACY_LIBRARY)).toBe(false);
  });
});

describe('ProjectSession current project', () => {
  it('has no current project at start', () => {
    expect(new ProjectSession(DOCUMENTS).currentProject).toBeNull();
  });

  // covers: FCT-0003
  it('holds the project that was just opened', () => {
    const session = new ProjectSession(DOCUMENTS);

    session.openProject(kitHorizon);

    expect(session.currentProject).toEqual(kitHorizon);
  });

  // covers: FCT-0003
  it('replaces the current project when another one is opened', () => {
    const session = new ProjectSession(DOCUMENTS);
    session.openProject(kitHorizon);

    session.openProject(legacyKit);

    expect(session.currentProject).toEqual(legacyKit);
  });

  it('has no current project once it is closed', () => {
    const session = new ProjectSession(DOCUMENTS);
    session.openProject(kitHorizon);

    session.closeProject();

    expect(session.currentProject).toBeNull();
  });

  it('accepts a close when no project is open', () => {
    const session = new ProjectSession(DOCUMENTS);

    session.closeProject();

    expect(session.currentProject).toBeNull();
  });
});
