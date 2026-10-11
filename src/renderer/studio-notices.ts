import type { ToastRequest } from './motion/toast.js';

export const REQUEST_REFUSED: ToastRequest = {
  tone: 'danger',
  title: 'Action impossible',
  message: "La demande a été refusée. Rien n'a été modifié.",
};
