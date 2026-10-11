import type { ComponentSummary } from '../../modules/sync/domain/library/component-summary.js';
import type { OpenedComponent } from '../../modules/sync/application/components/component-views.js';
import { IPC_CHANNELS } from '../ipc/channels.js';
import { readPathPayload, type PathPayload } from '../ipc/payloads.js';
import { guardedHandle, type AsyncPayloadReader, type IpcRegistry } from '../ipc/project-handlers.js';

export interface ComponentUseCases {
  list(): Promise<ComponentSummary[]>;
  resolve(path: string): Promise<string | null>;
  open(path: string): Promise<OpenedComponent>;
  lastOpened(): Promise<string | null>;
}

export interface ComponentHandlerDependencies {
  readonly components: ComponentUseCases;
  readonly isTrustedSender: (senderUrl: string) => boolean;
}

export function readEmptyPayload(payload: unknown): undefined | null {
  return payload === undefined ? undefined : null;
}

export function readListedComponent(components: ComponentUseCases): AsyncPayloadReader<PathPayload> {
  return async (payload) => {
    const request = readPathPayload(payload);
    if (request === null) return null;
    const listed = await components.resolve(request.path);
    return listed === null ? null : { path: listed };
  };
}

export function registerComponentHandlers(registry: IpcRegistry, deps: ComponentHandlerDependencies): void {
  const handle = guardedHandle(registry, deps.isTrustedSender);
  const { components } = deps;
  handle(IPC_CHANNELS.componentList, readEmptyPayload, () => components.list());
  handle(IPC_CHANNELS.componentOpen, readListedComponent(components), ({ path }) => components.open(path));
  handle(IPC_CHANNELS.componentLastOpened, readEmptyPayload, () => components.lastOpened());
}
