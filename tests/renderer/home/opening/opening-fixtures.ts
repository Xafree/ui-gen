import type {
  AdoptionProposal,
  OpenedProject,
} from '../../../../src/modules/project/application/open/project-outcomes.js';
import type { RecentProjectView } from '../../../../src/modules/project/application/recents/recent-projects-use-cases.js';
import type { UiGenApi } from '../../../../src/preload/ui-gen-api.js';
import { makeUiGenApi, type RecentsApi } from '../home-fixtures.js';

export type ProjectApi = UiGenApi['project'];

export const ATLAS_PATH = 'E:\\clients\\atlas-design-system';
export const REFUSED_MESSAGE = "La demande a été refusée. Rien n'a été modifié.";
export const REFUSED_NOTICE = { tone: 'danger', title: 'Action impossible', message: REFUSED_MESSAGE };

export function apiWith(project: Partial<ProjectApi>, recents: Partial<RecentsApi> = {}): UiGenApi {
  const base = makeUiGenApi(recents);
  return { ...base, project: { ...base.project, ...project } };
}

export function atlasProposal(): AdoptionProposal {
  return {
    path: ATLAS_PATH,
    name: 'Atlas',
    packageName: '@atlas/components',
    version: '2.3.1',
    componentCount: 12,
    componentsDir: 'src/components',
    tagPrefix: 'atlas-',
    exampleTag: 'atlas-button',
  };
}

export function atlasProject(tagPrefix: string, componentsDir: string): OpenedProject {
  return {
    path: ATLAS_PATH,
    name: 'Atlas',
    packageName: '@atlas/components',
    version: '2.3.1',
    settings: { formatVersion: 1, tagPrefix, componentsDir },
  };
}

export function atlasRecent(available: boolean): RecentProjectView {
  return { path: ATLAS_PATH, name: 'Atlas', openedAt: new Date(Date.now() - 3_600_000).toISOString(), available };
}

export function recordDetails(target: EventTarget, type: string): unknown[] {
  const details: unknown[] = [];
  target.addEventListener(type, (event: Event) => {
    if (event instanceof CustomEvent) {
      const detail: unknown = event.detail;
      details.push(detail);
    }
  });
  return details;
}

export function closedChannel(): Promise<never> {
  return Promise.reject(new Error('IPC channel closed'));
}
