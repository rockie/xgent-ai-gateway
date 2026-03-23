import { useRouter } from '@tanstack/react-router'
import { useDeregisterService } from '@/lib/services'
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

interface DeregisterDialogProps {
  serviceName: string
  open: boolean
  onOpenChange: (open: boolean) => void
}

export function DeregisterDialog({
  serviceName,
  open,
  onOpenChange,
}: DeregisterDialogProps) {
  const router = useRouter()
  const deregisterMutation = useDeregisterService()

  function handleDeregister() {
    deregisterMutation.mutate(serviceName, {
      onSuccess: () => {
        onOpenChange(false)
        router.navigate({ to: '/services' })
      },
    })
  }

  return (
    <AlertDialog open={open} onOpenChange={onOpenChange}>
      <AlertDialogContent>
        <AlertDialogHeader>
          <AlertDialogTitle>Deregister service?</AlertDialogTitle>
          <AlertDialogDescription>
            This will permanently remove &ldquo;{serviceName}&rdquo; and
            disconnect all its nodes. This action cannot be undone.
          </AlertDialogDescription>
        </AlertDialogHeader>
        <AlertDialogFooter>
          <AlertDialogCancel>Cancel</AlertDialogCancel>
          <AlertDialogAction
            className="bg-destructive text-destructive-foreground hover:bg-destructive/90"
            disabled={deregisterMutation.isPending}
            onClick={handleDeregister}
          >
            {deregisterMutation.isPending ? 'Deregistering...' : 'Deregister'}
          </AlertDialogAction>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  )
}
