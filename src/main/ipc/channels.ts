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
  componentList: 'component:list',
  componentOpen: 'component:open',
  componentLastOpened: 'component:last-opened',
} as const;

export const PUSH_CHANNELS = {
  componentChanged: 'component:changed',
  componentStatus: 'component:status',
} as const;

export type PushChannel = (typeof PUSH_CHANNELS)[keyof typeof PUSH_CHANNELS];

export type IpcChannel = (typeof IPC_CHANNELS)[keyof typeof IPC_CHANNELS];
