import { parentPort, workerData } from 'node:worker_threads';
import { readPropsInline } from './props.mjs';

const { components, notes } = await readPropsInline(workerData.installed, workerData.limits);
parentPort.postMessage({ components: [...components], notes });
