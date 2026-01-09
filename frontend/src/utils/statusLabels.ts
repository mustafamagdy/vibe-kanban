import { TaskStatus } from 'shared/types';

export const statusLabels: Record<TaskStatus, string> = {
  todo: 'To Do',
  inprogress: 'In Progress',
  testing: 'Testing',
  inreview: 'AI Review',
  humanreview: 'Human Review',
  done: 'Done',
  cancelled: 'Cancelled',
};

export const statusBoardColors: Record<TaskStatus, string> = {
  todo: '--neutral-foreground',
  inprogress: '--info',
  testing: '--info',
  inreview: '--warning',
  humanreview: '--warning',
  done: '--success',
  cancelled: '--destructive',
};
