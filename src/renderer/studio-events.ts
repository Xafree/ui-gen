import type { OpenedProject } from '../modules/project/application/open/project-outcomes.js';
import type { ToastRequest } from './motion/toast.js';

export const PROJECT_OPENED = 'project-opened';
export const PROJECT_CLOSED = 'project-closed';
export const PROJECT_CREATED = 'project-created';
export const PROJECT_ADOPTED = 'project-adopted';
export const NOTICE_REQUESTED = 'notice-requested';

export interface ProjectOpenedDetail {
  readonly project: OpenedProject;
  readonly notice: ToastRequest | null;
}

export interface ProjectClosedDetail {
  readonly project: OpenedProject;
  readonly reason: 'switch' | 'close';
}

export interface ProjectCreatedDetail {
  readonly project: OpenedProject;
}

export interface ProjectAdoptedDetail {
  readonly project: OpenedProject;
}

export interface NoticeRequestedDetail {
  readonly notice: ToastRequest;
}
