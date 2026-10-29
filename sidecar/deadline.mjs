import { ServiceError } from './protocol.mjs';

export function startBudget(ms, message) {
  const end = Date.now() + ms;
  const remaining = () => Math.max(0, end - Date.now());
  const bound = (promise) => {
    let timer;
    const expired = new Promise((_, reject) => {
      timer = setTimeout(() => reject(new ServiceError('render', message)), remaining());
    });
    return Promise.race([promise, expired]).finally(() => clearTimeout(timer));
  };
  return { remaining, bound };
}
