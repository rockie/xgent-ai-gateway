import { useState } from 'react'
import { Copy, Check, MoreHorizontal, ListTodo } from 'lucide-react'
import type { TaskSummary } from '@/lib/tasks'
import { canCancel } from '@/lib/tasks'
import { relativeTime } from '@/lib/services'
import { TaskStatusBadge } from '@/components/task-status-badge'
import { TaskCancelDialog } from '@/components/task-cancel-dialog'
import { EmptyState } from '@/components/empty-state'
import { Button } from '@/components/ui/button'
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from '@/components/ui/table'
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'

interface TaskTableProps {
  tasks: TaskSummary[]
  onRowClick: (taskId: string) => void
}

export function TaskTable({ tasks, onRowClick }: TaskTableProps) {
  const [cancelTaskId, setCancelTaskId] = useState<string | null>(null)
  const [cancelDialogOpen, setCancelDialogOpen] = useState(false)
  const [copiedId, setCopiedId] = useState<string | null>(null)

  const handleCopyId = async (taskId: string) => {
    await navigator.clipboard.writeText(taskId)
    setCopiedId(taskId)
    setTimeout(() => setCopiedId(null), 2000)
  }

  if (tasks.length === 0) {
    return (
      <EmptyState
        icon={ListTodo}
        heading="No tasks found"
        description="No tasks match the current filters. Try adjusting your filters or wait for new tasks to be submitted."
      />
    )
  }

  return (
    <>
      <Table>
        <TableHeader>
          <TableRow>
            <TableHead>Task ID</TableHead>
            <TableHead>Service</TableHead>
            <TableHead>Status</TableHead>
            <TableHead>Created</TableHead>
            <TableHead>Completed</TableHead>
            <TableHead className="w-[50px]">
              <span className="sr-only">Actions</span>
            </TableHead>
          </TableRow>
        </TableHeader>
        <TableBody>
          {tasks.map((task) => (
            <TableRow
              key={task.task_id}
              className="cursor-pointer hover:bg-muted/50"
              onClick={() => onRowClick(task.task_id)}
            >
              <TableCell>
                <div className="flex items-center gap-1">
                  <span className="font-mono text-sm">
                    {task.task_id.slice(0, 8)}
                  </span>
                  <Button
                    variant="ghost"
                    size="icon"
                    className="h-6 w-6 opacity-0 group-hover:opacity-100 hover:opacity-100"
                    onClick={(e) => {
                      e.stopPropagation()
                      handleCopyId(task.task_id)
                    }}
                  >
                    {copiedId === task.task_id ? (
                      <Check className="h-3 w-3" />
                    ) : (
                      <Copy className="h-3 w-3" />
                    )}
                  </Button>
                </div>
              </TableCell>
              <TableCell>{task.service}</TableCell>
              <TableCell>
                <TaskStatusBadge state={task.state} />
              </TableCell>
              <TableCell title={task.created_at}>
                {relativeTime(task.created_at)}
              </TableCell>
              <TableCell title={task.completed_at || undefined}>
                {task.completed_at ? relativeTime(task.completed_at) : '-'}
              </TableCell>
              <TableCell>
                <DropdownMenu>
                  <DropdownMenuTrigger asChild>
                    <Button
                      variant="ghost"
                      size="icon"
                      className="h-8 w-8"
                      onClick={(e) => e.stopPropagation()}
                    >
                      <MoreHorizontal className="h-4 w-4" />
                      <span className="sr-only">Open menu</span>
                    </Button>
                  </DropdownMenuTrigger>
                  <DropdownMenuContent align="end">
                    <DropdownMenuItem
                      onClick={(e) => {
                        e.stopPropagation()
                        onRowClick(task.task_id)
                      }}
                    >
                      View details
                    </DropdownMenuItem>
                    <DropdownMenuItem
                      onClick={(e) => {
                        e.stopPropagation()
                        handleCopyId(task.task_id)
                      }}
                    >
                      Copy task ID
                    </DropdownMenuItem>
                    {canCancel(task.state) && (
                      <DropdownMenuItem
                        className="text-destructive"
                        onClick={(e) => {
                          e.stopPropagation()
                          setCancelTaskId(task.task_id)
                          setCancelDialogOpen(true)
                        }}
                      >
                        Cancel task
                      </DropdownMenuItem>
                    )}
                  </DropdownMenuContent>
                </DropdownMenu>
              </TableCell>
            </TableRow>
          ))}
        </TableBody>
      </Table>

      {cancelTaskId && (
        <TaskCancelDialog
          taskId={cancelTaskId}
          shortId={cancelTaskId.slice(0, 8)}
          open={cancelDialogOpen}
          onOpenChange={(open) => {
            setCancelDialogOpen(open)
            if (!open) setCancelTaskId(null)
          }}
        />
      )}
    </>
  )
}
