import type { RecentProject } from '../../modules/project/domain/recents/recent-projects.js';

function keyOf({ path, name, openedAt }: RecentProject): string {
  return JSON.stringify([path, name, openedAt]);
}

export class ForgottenRecents {
  readonly #keys = new Set<string>();

  remember(projects: readonly RecentProject[]): void {
    for (const project of projects) this.#keys.add(keyOf(project));
  }

  isForgotten(project: RecentProject): boolean {
    return this.#keys.has(keyOf(project));
  }
}
