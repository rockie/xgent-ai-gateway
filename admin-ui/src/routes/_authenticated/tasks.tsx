import { createFileRoute } from '@tanstack/react-router'
import { useState, useRef, useCallback } from 'react'
import { ListTodo, Search } from 'lucide-react'
import { useTasks, type TaskFilters } from '@/lib/tasks'
import { useServices } from '@/lib/services'
import { TaskTable } from '@/components/task-table'
import { TaskDetailSheet } from '@/components/task-detail-sheet'
import { EmptyState } from '@/components/empty-state'
import { ErrorAlert } from '@/components/error-alert'
import { PageSkeleton } from '@/components/page-skeleton'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from '@/components/ui/popover'
import { Checkbox } from '@/components/ui/checkbox'

export const Route = createFileRoute('/_authenticated/tasks')({
  component: TasksPage,
})

const STATUS_OPTIONS = ['pending', 'assigned', 'running', 'completed', 'failed']
const PAGE_SIZE_OPTIONS = [10, 25, 50]

function TasksPage() {
  const [filters, setFilters] = useState<TaskFilters>({ page_size: 25 })
  const [selectedTaskId, setSelectedTaskId] = useState<string | null>(null)
  const [sheetOpen, setSheetOpen] = useState(false)
  const [selectedStatuses, setSelectedStatuses] = useState<Set<string>>(
    new Set(),
  )
  const cursorHistory = useRef<string[]>([])
  const debounceTimer = useRef<ReturnType<typeof setTimeout> | null>(null)

  const { data, isLoading, isError, error, refetch } = useTasks(filters)
  const { data: servicesData } = useServices()

  const resetCursor = useCallback(
    (updater: (prev: TaskFilters) => TaskFilters) => {
      cursorHistory.current = []
      setFilters((prev) => {
        const next = updater(prev)
        return { ...next, cursor: undefined }
      })
    },
    [],
  )

  const handleSearchChange = (value: string) => {
    if (debounceTimer.current) clearTimeout(debounceTimer.current)
    debounceTimer.current = setTimeout(() => {
      resetCursor((prev) => ({
        ...prev,
        task_id: value || undefined,
      }))
    }, 300)
  }

  const handleServiceChange = (value: string | null) => {
    resetCursor((prev) => ({
      ...prev,
      service: !value || value === '__all__' ? undefined : value,
    }))
  }

  const handlePageSizeChange = (value: string | null) => {
    if (!value) return
    resetCursor((prev) => ({
      ...prev,
      page_size: Number(value),
    }))
  }

  const handleStatusToggle = (status: string, checked: boolean) => {
    setSelectedStatuses((prev) => {
      const next = new Set(prev)
      if (checked) {
        next.add(status)
      } else {
        next.delete(status)
      }
      const statusStr = next.size > 0 ? Array.from(next).join(',') : undefined
      resetCursor((p) => ({ ...p, status: statusStr }))
      return next
    })
  }

  const handleNext = () => {
    if (data?.cursor) {
      cursorHistory.current.push(filters.cursor ?? '')
      setFilters((prev) => ({ ...prev, cursor: data.cursor! }))
    }
  }

  const handlePrevious = () => {
    const prev = cursorHistory.current.pop()
    setFilters((f) => ({
      ...f,
      cursor: prev || undefined,
    }))
  }

  const handleRowClick = (taskId: string) => {
    setSelectedTaskId(taskId)
    setSheetOpen(true)
  }

  const hasActiveFilters =
    !!filters.service || !!filters.status || !!filters.task_id

  return (
    <div className="space-y-6">
      <h1 className="text-xl font-semibold">Tasks</h1>

      {/* Filter bar */}
      <div className="flex flex-wrap items-center gap-2">
        <Input
          placeholder="Search by task ID..."
          className="w-64"
          onChange={(e) => handleSearchChange(e.target.value)}
        />

        <Select
          value={filters.service ?? '__all__'}
          onValueChange={handleServiceChange}
        >
          <SelectTrigger className="w-[180px]">
            <SelectValue placeholder="All services" />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="__all__">All services</SelectItem>
            {servicesData?.services.map((svc) => (
              <SelectItem key={svc.name} value={svc.name}>
                {svc.name}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>

        <Popover>
          <PopoverTrigger
            render={<Button variant="outline" className="gap-1" />}
          >
            Status
            {selectedStatuses.size > 0 && (
              <span className="ml-1 rounded-full bg-primary text-primary-foreground text-xs px-1.5 py-0.5">
                {selectedStatuses.size}
              </span>
            )}
          </PopoverTrigger>
          <PopoverContent className="w-48 p-3" align="start">
            <div className="space-y-2">
              {STATUS_OPTIONS.map((status) => (
                <label
                  key={status}
                  className="flex items-center gap-2 cursor-pointer"
                >
                  <Checkbox
                    checked={selectedStatuses.has(status)}
                    onCheckedChange={(checked) =>
                      handleStatusToggle(status, checked === true)
                    }
                  />
                  <span className="text-sm capitalize">{status}</span>
                </label>
              ))}
            </div>
          </PopoverContent>
        </Popover>

        <Select
          value={String(filters.page_size ?? 25)}
          onValueChange={handlePageSizeChange}
        >
          <SelectTrigger className="w-[100px]">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            {PAGE_SIZE_OPTIONS.map((size) => (
              <SelectItem key={size} value={String(size)}>
                {size} / page
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      </div>

      {/* Loading state */}
      {isLoading && <PageSkeleton lines={8} />}

      {/* Error state */}
      {isError && error && (
        <ErrorAlert message={error.message} onRetry={refetch} />
      )}

      {/* Data states */}
      {data && data.tasks.length === 0 && !hasActiveFilters && (
        <EmptyState
          icon={ListTodo}
          heading="No tasks"
          description="No tasks have been submitted yet."
        />
      )}

      {data && data.tasks.length === 0 && hasActiveFilters && (
        <EmptyState
          icon={Search}
          heading="No matching tasks"
          description="No tasks match your filters. Try adjusting your search criteria."
        />
      )}

      {data && data.tasks.length > 0 && (
        <>
          <TaskTable tasks={data.tasks} onRowClick={handleRowClick} />

          {/* Pagination bar */}
          <div className="flex items-center justify-between">
            <Button
              variant="outline"
              size="sm"
              disabled={cursorHistory.current.length === 0 && !filters.cursor}
              onClick={handlePrevious}
            >
              Previous
            </Button>
            <Button
              variant="outline"
              size="sm"
              disabled={!data.cursor}
              onClick={handleNext}
            >
              Next
            </Button>
          </div>
        </>
      )}

      {/* Detail sheet */}
      <TaskDetailSheet
        taskId={selectedTaskId}
        open={sheetOpen}
        onOpenChange={setSheetOpen}
      />
    </div>
  )
}
