import { contextBridge } from 'electron';
import type { ApiUiGen } from './api-ui-gen.js';

const api: ApiUiGen = Object.freeze({
  plateforme: process.platform,
});

contextBridge.exposeInMainWorld('uiGen', api);
