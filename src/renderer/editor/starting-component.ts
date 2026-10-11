import type { OpenedComponent } from '../../modules/sync/application/components/component-views.js';
import type { UiGenApi } from '../../preload/ui-gen-api.js';

export async function openStartingComponent(components: UiGenApi['components']): Promise<OpenedComponent | null> {
  try {
    const lastOpened = await components.lastOpened();
    const path = lastOpened ?? (await components.list())[0]?.path ?? null;
    return path === null ? null : await components.open(path);
  } catch (error) {
    console.warn('The component to show in the editor could not be opened', error);
    return null;
  }
}
