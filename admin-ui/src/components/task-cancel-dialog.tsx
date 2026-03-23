import { useCancelTask } from '@/lib/tasks'
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from '@/components/ui/alert-dialog'

interface TaskCancelDialogProps {
  taskId: string
  shortId: string
  open: boolean
  onOpenChange: (open: boolean) => void
}

export function TaskCancelDialog({
  taskId,
  shortId,
  open,
  onOpenChange,
}: TaskCancelDialogProps) {
  const cancelMutation = useCancelTask()

  function handleConfirm() {
    cancelMutation.mutate(taskId, {
      onSuccess: () => {
        onOpenChange(false)
      },
    })
  }

  return (
    <AlertDialog open={open} onOpenChange={onOpenChange}>
      <AlertDialogContent>
        <AlertDialogHeader>
          <AlertDialogTitle>Cancel task {shortId}?</AlertDialogTitle>
          <AlertDialogDescription>
            This will mark it as failed for the client. This action cannot be
            undone.
          </AlertDialogDescription>
        </AlertDialogHeader>
        <AlertDialogFooter>
          <AlertDialogCancel>Cancel</AlertDialogCancel>
          <AlertDialogAction
            className="bg-destructive text-destructive-foreground hover:bg-destructive/90"
            disabled={cancelMutation.isPending}
            onClick={handleConfirm}
          >
            {cancelMutation.isPending ? 'Cancelling...' : 'Confirm'}
          </AlertDialogAction>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  )
}
