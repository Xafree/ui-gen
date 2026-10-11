import { utilityProcess } from 'electron';
import type { ComputeProcess } from '../../modules/sync/adapters/utility-process/compute-client.js';

const SERVICE_NAME = 'UI-GEN analysis';

export function forkComputeProcess(modulePath: string): ComputeProcess {
  const child = utilityProcess.fork(modulePath, [], { serviceName: SERVICE_NAME });
  return {
    send: (request) => {
      child.postMessage(request);
    },
    onMessage: (listener) => {
      child.on('message', listener);
    },
    onExit: (listener) => {
      child.on('exit', listener);
    },
    kill: () => {
      child.kill();
    },
  };
}
