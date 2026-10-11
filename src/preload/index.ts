import { contextBridge } from 'electron';
import type { UiGenApi } from './ui-gen-api.js';

const api: Pick<UiGenApi, 'platform'> = Object.freeze({
  platform: process.platform,
});

contextBridge.exposeInMainWorld('uiGen', api);
