import type { OpenedProject } from '../../modules/project/application/open/project-outcomes.js';

function comparable(path: string): string {
  return path.toLowerCase();
}

export class ProjectSession {
  readonly defaultLocation: string;
  readonly #grantedLocations: Set<string>;
  #pendingAdoption: string | null = null;
  #currentProject: OpenedProject | null = null;

  constructor(defaultLocation: string) {
    this.defaultLocation = defaultLocation;
    this.#grantedLocations = new Set([comparable(defaultLocation)]);
  }

  get currentProject(): OpenedProject | null {
    return this.#currentProject;
  }

  grantLocation(path: string): void {
    this.#grantedLocations.add(comparable(path));
  }

  isLocationGranted(path: string): boolean {
    return this.#grantedLocations.has(comparable(path));
  }

  setPendingAdoption(path: string | null): void {
    this.#pendingAdoption = path === null ? null : comparable(path);
  }

  isPendingAdoption(path: string): boolean {
    return this.#pendingAdoption === comparable(path);
  }

  openProject(project: OpenedProject): void {
    this.#currentProject = project;
    this.#pendingAdoption = null;
  }

  closeProject(): void {
    this.#currentProject = null;
  }
}
