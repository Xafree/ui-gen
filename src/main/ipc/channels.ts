export const IPC_CHANNELS = {
  recentsList: 'recents:list',
  recentsRemove: 'recents:remove',
  recentsClear: 'recents:clear',
  recentsRestore: 'recents:restore',
  recentsOpen: 'recents:open',
  projectDefaultLocation: 'project:default-location',
  projectChooseLocation: 'project:choose-location',
  projectCreate: 'project:create',
  projectOpenFolder: 'project:open-folder',
  projectAdopt: 'project:adopt',
  projectClose: 'project:close',
} as const;

export type IpcChannel = (typeof IPC_CHANNELS)[keyof typeof IPC_CHANNELS];
