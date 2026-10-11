import type { PushChannel } from '../ipc/channels.js';

export interface StudioContents {
  isDestroyed(): boolean;
  send(channel: string, payload: unknown): void;
}

export type StudioPublish = (channel: PushChannel, payload: unknown) => void;

export function createStudioPublisher(studioContents: () => StudioContents | null): StudioPublish {
  return (channel, payload) => {
    const contents = studioContents();
    if (contents === null || contents.isDestroyed()) return;
    contents.send(channel, payload);
  };
}
