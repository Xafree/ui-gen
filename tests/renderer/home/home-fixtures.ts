import type { OpenedProject } from '../../../src/modules/project/application/open/project-outcomes.js';
import type {
  RecentProjectsListing,
  RecentProjectView,
} from '../../../src/modules/project/application/recents/recent-projects-use-cases.js';
import type { UiGenApi } from '../../../src/preload/ui-gen-api.js';

export type RecentsApi = UiGenApi['recents'];

const MINUTE = 60_000;
const HOUR = 60 * MINUTE;
const DAY = 24 * HOUR;

function notExpected(call: string): () => Promise<never> {
  return () => Promise.reject(new Error(`${call} is not expected in this test`));
}

export function makeUiGenApi(recents: Partial<RecentsApi>): UiGenApi {
  return {
    platform: 'win32',
    recents: {
      list: recents.list ?? notExpected('recents.list'),
      remove: recents.remove ?? notExpected('recents.remove'),
      open: recents.open ?? notExpected('recents.open'),
      clear: recents.clear ?? notExpected('recents.clear'),
      restore: recents.restore ?? notExpected('recents.restore'),
    },
    project: {
      defaultLocation: notExpected('project.defaultLocation'),
      chooseLocation: notExpected('project.chooseLocation'),
      create: notExpected('project.create'),
      openFolder: notExpected('project.openFolder'),
      adopt: notExpected('project.adopt'),
      close: notExpected('project.close'),
    },
    components: {
      list: notExpected('components.list'),
      open: notExpected('components.open'),
      lastOpened: notExpected('components.lastOpened'),
      onChanged: () => () => undefined,
      onStatus: () => () => undefined,
    },
  };
}

function openedAgo(milliseconds: number): string {
  return new Date(Date.now() - milliseconds).toISOString();
}

export function listing(projects: RecentProjectView[], unreadable = false): RecentProjectsListing {
  return { projects, unreadable };
}

export function kitHorizon(): RecentProjectView {
  return { path: 'E:\\projets\\kit-horizon', name: 'Kit Horizon', openedAt: openedAgo(6 * MINUTE), available: true };
}

export function lumenUi(): RecentProjectView {
  return { path: 'E:\\projets\\lumen-ui', name: 'Lumen UI', openedAt: openedAgo(DAY + 3 * HOUR), available: true };
}

export function composantsVerdure(): RecentProjectView {
  return {
    path: 'E:\\clients\\verdure\\composants',
    name: 'Composants Verdure',
    openedAt: openedAgo(4 * DAY),
    available: true,
  };
}

export function prototypePivot(): RecentProjectView {
  return { path: 'D:\\archives\\pivot-ui', name: 'Prototype Pivot', openedAt: openedAgo(23 * DAY), available: false };
}

export function atelierNova(available: boolean): RecentProjectView {
  return { path: 'D:\\archives\\nova-ui', name: 'Atelier Nova', openedAt: openedAgo(40 * DAY), available };
}

export function kitHorizonProject(): OpenedProject {
  return {
    path: 'E:\\projets\\kit-horizon',
    name: 'Kit Horizon',
    packageName: '@horizon/ui',
    version: '0.4.0',
    settings: { formatVersion: 1, tagPrefix: 'hz-', componentsDir: 'src/components' },
  };
}
