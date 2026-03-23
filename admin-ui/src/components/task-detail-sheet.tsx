import { useState } from 'react'
import { Copy, Check } from 'lucide-react'
import { useTaskDetail, canCancel } from '@/lib/tasks'
import { relativeTime } from '@/lib/services'
import { TaskStatusBadge } from '@/components/task-status-badge'
import { JsonViewer } from '@/components/json-viewer'
import { TaskCancelDialog } from '@/components/task-cancel-dialog'
import { ErrorAlert } from '@/components/error-alert'
import { Skeleton } from '@/components/ui/skeleton'
import { Button } from '@/components/ui/button'
import {
  Sheet,
  SheetContent,
  SheetHeader,
  SheetTitle,
} from '@/components/ui/sheet'

interface TaskDetailSheetProps {
  taskId: string | null
  open: boolean
  onOpenChange: (open: boolean) => void
}

export function TaskDetailSheet({
  taskId,
  open,
  onOpenChange,
}: TaskDetailSheetProps) {
  const { data: task, isLoading, error, refetch } = useTaskDetail(taskId ?? '')
  const [cancelOpen, setCancelOpen] = useState(false)
  const [copied, setCopied] = useState(false)

  const handleCopyId = async () => {
    if (!taskId) return
    await navigator.clipboard.writeText(taskId)
    setCopied(true)
    setTimeout(() => setCopied(false), 2000)
  }

  return (
    <>
      <Sheet open={open} onOpenChange={onOpenChange}>
        <SheetContent
          side="right"
          className="sm:max-w-[50vw] sm:w-[50vw] overflow-y-auto"
        >
          <SheetHeader>
            <SheetTitle>Task Details</SheetTitle>
          </SheetHeader>

          <div className="px-4 pb-4 space-y-6">
            {isLoading && (
              <div className="space-y-4">
                <Skeleton className="h-6 w-3/4" />
                <Skeleton className="h-4 w-1/2" />
                <Skeleton className="h-4 w-full" />
                <Skeleton className="h-32 w-full" />
              </div>
            )}

            {error && (
              <ErrorAlert
                message={error.message}
                onRetry={() => refetch()}
              />
            )}

            {task && (
              <>
                {/* Section 1: Task info header */}
                <div className="space-y-3">
                  <div className="flex items-center gap-2">
                    <code className="font-mono text-sm break-all flex-1">
                      {task.task_id}
                    </code>
                    <Button
                      variant="ghost"
                      size="icon"
                      className="h-7 w-7 shrink-0"
                      onClick={handleCopyId}
                    >
                      {copied ? (
                        <Check className="h-3.5 w-3.5" />
                      ) : (
                        <Copy className="h-3.5 w-3.5" />
                      )}
                    </Button>
                  </div>
                  <div className="flex items-center gap-3">
                    <TaskStatusBadge state={task.state} />
                    <span className="text-sm text-muted-foreground">
                      {task.service}
                    </span>
                  </div>
                  <div className="grid grid-cols-2 gap-2 text-sm">
                    <div>
                      <span className="text-muted-foreground">Created: </span>
                      <span title={task.created_at}>
                        {relativeTime(task.created_at)}
                      </span>
                    </div>
                    {task.completed_at && (
                      <div>
                        <span className="text-muted-foreground">
                          Completed:{' '}
                        </span>
                        <span title={task.completed_at}>
                          {relativeTime(task.completed_at)}
                        </span>
                      </div>
                    )}
                  </div>
                </div>

                {/* Section 2: Metadata */}
                <div>
                  <h3 className="text-sm font-medium mb-2">Metadata</h3>
                  {Object.keys(task.metadata).length > 0 ? (
                    <dl className="grid grid-cols-[auto_1fr] gap-x-4 gap-y-1 text-sm">
                      {Object.entries(task.metadata).map(([key, value]) => (
                        <div key={key} className="contents">
                          <dt className="text-muted-foreground font-mono">
                            {key}
                          </dt>
                          <dd className="break-all">{value}</dd>
                        </div>
                      ))}
                    </dl>
                  ) : (
                    <p className="text-sm text-muted-foreground">
                      No metadata
                    </p>
                  )}
                </div>

                {/* Section 3: Payload */}
                <div>
                  <h3 className="text-sm font-medium mb-2">Payload</h3>
                  <JsonViewer base64Data={task.payload} label="payload" />
                </div>

                {/* Section 4: Result */}
                <div>
                  <h3 className="text-sm font-medium mb-2">Result</h3>
                  {task.error_message && (
                    <div className="rounded-md bg-red-500/10 border border-red-500/30 p-3 mb-3 text-sm text-red-500">
                      {task.error_message}
                    </div>
                  )}
                  <JsonViewer base64Data={task.result} label="result" />
                </div>

                {/* Cancel button */}
                {canCancel(task.state) && (
                  <div className="pt-2">
                    <Button
                      variant="destructive"
                      onClick={() => setCancelOpen(true)}
                    >
                      Cancel Task
                    </Button>
                  </div>
                )}
              </>
            )}
          </div>
        </SheetContent>
      </Sheet>

      {taskId && (
        <TaskCancelDialog
          taskId={taskId}
          shortId={taskId.slice(0, 8)}
          open={cancelOpen}
          onOpenChange={setCancelOpen}
        />
      )}
    </>
  )
}
