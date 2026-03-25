import { useState } from 'react'
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
  DialogFooter,
} from '@/components/ui/dialog'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Separator } from '@/components/ui/separator'
import { useRegisterService, type RegisterServiceRequest } from '@/lib/services'

interface ServiceRegistrationDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
}

export function ServiceRegistrationDialog({
  open,
  onOpenChange,
}: ServiceRegistrationDialogProps) {
  const registerMutation = useRegisterService()

  const [name, setName] = useState('')
  const [description, setDescription] = useState('')
  const [taskTimeoutSecs, setTaskTimeoutSecs] = useState('')
  const [maxNodes, setMaxNodes] = useState('')
  const [nodeStaleAfterSecs, setNodeStaleAfterSecs] = useState('')
  const [drainTimeoutSecs, setDrainTimeoutSecs] = useState('')

  function resetForm() {
    setName('')
    setDescription('')
    setTaskTimeoutSecs('')
    setMaxNodes('')
    setNodeStaleAfterSecs('')
    setDrainTimeoutSecs('')
  }

  function handleSubmit(e: React.FormEvent) {
    e.preventDefault()

    const request: RegisterServiceRequest = {
      name: name.trim(),
    }

    if (description.trim()) request.description = description.trim()
    if (taskTimeoutSecs) request.task_timeout_secs = Number(taskTimeoutSecs)
    if (maxNodes) request.max_nodes = Number(maxNodes)
    if (nodeStaleAfterSecs)
      request.node_stale_after_secs = Number(nodeStaleAfterSecs)
    if (drainTimeoutSecs)
      request.drain_timeout_secs = Number(drainTimeoutSecs)

    registerMutation.mutate(request, {
      onSuccess: () => {
        onOpenChange(false)
        resetForm()
      },
    })
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>Register Service</DialogTitle>
          <DialogDescription>
            Register a new service with the gateway. Only the name is required.
          </DialogDescription>
        </DialogHeader>
        <form onSubmit={handleSubmit}>
          <div className="grid gap-4 py-4">
            <div className="grid gap-2">
              <Label htmlFor="service-name">Service Name</Label>
              <Input
                id="service-name"
                placeholder="my-service"
                value={name}
                onChange={(e) => setName(e.target.value)}
                required
              />
            </div>
            <div className="grid gap-2">
              <Label htmlFor="service-description">Description</Label>
              <Input
                id="service-description"
                placeholder="Optional description"
                value={description}
                onChange={(e) => setDescription(e.target.value)}
              />
            </div>

            <Separator />
            <p className="text-sm font-medium text-muted-foreground">
              Advanced Settings
            </p>

            <div className="grid gap-2">
              <Label htmlFor="task-timeout">Task Timeout (seconds)</Label>
              <Input
                id="task-timeout"
                type="number"
                placeholder="300"
                value={taskTimeoutSecs}
                onChange={(e) => setTaskTimeoutSecs(e.target.value)}
              />
            </div>
            <div className="grid gap-2">
              <Label htmlFor="max-nodes">Max Nodes</Label>
              <Input
                id="max-nodes"
                type="number"
                placeholder="Unlimited"
                value={maxNodes}
                onChange={(e) => setMaxNodes(e.target.value)}
              />
            </div>
            <div className="grid gap-2">
              <Label htmlFor="node-stale-after">
                Node Stale After (seconds)
              </Label>
              <Input
                id="node-stale-after"
                type="number"
                placeholder="60"
                value={nodeStaleAfterSecs}
                onChange={(e) => setNodeStaleAfterSecs(e.target.value)}
              />
            </div>
            <div className="grid gap-2">
              <Label htmlFor="drain-timeout">Drain Timeout (seconds)</Label>
              <Input
                id="drain-timeout"
                type="number"
                placeholder="30"
                value={drainTimeoutSecs}
                onChange={(e) => setDrainTimeoutSecs(e.target.value)}
              />
            </div>
          </div>
          <DialogFooter>
            <Button
              type="button"
              variant="outline"
              onClick={() => onOpenChange(false)}
            >
              Cancel
            </Button>
            <Button type="submit" disabled={registerMutation.isPending}>
              {registerMutation.isPending ? 'Registering...' : 'Register Service'}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  )
}
