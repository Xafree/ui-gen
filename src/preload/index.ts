import { contextBridge } from 'electron';
import type { UiGenApi } from './ui-gen-api.js';

const api: UiGenApi = Object.freeze({
  platform: process.platform,
});

contextBridge.exposeInMainWorld('uiGen', api);
