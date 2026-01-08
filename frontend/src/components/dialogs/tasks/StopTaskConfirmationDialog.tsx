import { useState } from 'react';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog';
import { Button } from '@/components/ui/button';
import { Alert } from '@/components/ui/alert';
import { attemptsApi } from '@/lib/api';
import NiceModal, { useModal } from '@ebay/nice-modal-react';
import { defineModal } from '@/lib/modals';

export interface StopTaskConfirmationDialogProps {
  taskTitle: string;
  attemptId: string;
}

const StopTaskConfirmationDialogImpl =
  NiceModal.create<StopTaskConfirmationDialogProps>(({ taskTitle, attemptId }) => {
    const modal = useModal();
    const [isStopping, setIsStopping] = useState(false);
    const [error, setError] = useState<string | null>(null);

    const handleConfirmStop = async () => {
      setIsStopping(true);
      setError(null);

      try {
        await attemptsApi.stop(attemptId);
        modal.resolve();
        modal.hide();
      } catch (err: unknown) {
        const errorMessage =
          err instanceof Error ? err.message : 'Failed to stop task';
        setError(errorMessage);
      } finally {
        setIsStopping(false);
      }
    };

    const handleCancelStop = () => {
      modal.reject();
      modal.hide();
    };

    return (
      <Dialog
        open={modal.visible}
        onOpenChange={(open) => !open && handleCancelStop()}
      >
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Stop Task</DialogTitle>
            <DialogDescription>
              Are you sure you want to stop{' '}
              <span className="font-semibold">"{taskTitle}"</span>?
            </DialogDescription>
          </DialogHeader>

          <Alert variant="default" className="mb-4">
            <strong>Note:</strong> This will stop the running attempt. You can
            start a new attempt later.
          </Alert>

          {error && (
            <Alert variant="destructive" className="mb-4">
              {error}
            </Alert>
          )}

          <DialogFooter>
            <Button
              variant="outline"
              onClick={handleCancelStop}
              disabled={isStopping}
              autoFocus
            >
              Cancel
            </Button>
            <Button
              variant="destructive"
              onClick={handleConfirmStop}
              disabled={isStopping}
            >
              {isStopping ? 'Stopping...' : 'Stop Task'}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    );
  });

export const StopTaskConfirmationDialog = defineModal<
  StopTaskConfirmationDialogProps,
  void
>(StopTaskConfirmationDialogImpl);
