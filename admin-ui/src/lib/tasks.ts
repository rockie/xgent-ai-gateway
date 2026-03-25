import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { apiClient } from './api'
import { useAutoRefresh } from '@/hooks/use-auto-refresh'
import { toast } from 'sonner'

// Types matching backend response shapes exactly

export interface TaskSummary {
  task_id: string
  state: string
  service: string
  created_at: string
  completed_at: string
}

export interface ListTasksResponse {
  tasks: TaskSummary[]
  cursor: string | null
}

export interface TaskDetailResponse {
  task_id: string
  state: string
  service: string
  payload: unknown      // JSON value from backend
  result: unknown       // JSON value from backend
  error_message: string
  metadata: Record<string, string>
  created_at: string
  completed_at: string
  stream_id: string
}

export interface TaskFilters {
  cursor?: string
  page_size?: number
  service?: string
  status?: string   // comma-separated: "pending,running"
  task_id?: string  // direct lookup
}

// Hooks

export function useTasks(filters: TaskFilters) {
  const { effectiveInterval } = useAutoRefresh()
  return useQuery({
    queryKey: ['tasks', filters],
    queryFn: () => {
      const params = new URLSearchParams()
      if (filters.cursor) params.set('cursor', filters.cursor)
      if (filters.page_size) params.set('page_size', String(filters.page_size))
      if (filters.service) params.set('service', filters.service)
      if (filters.status) params.set('status', filters.status)
      if (filters.task_id) params.set('task_id', filters.task_id)
      const qs = params.toString()
      return apiClient<ListTasksResponse>(`/v1/admin/tasks${qs ? `?${qs}` : ''}`)
    },
    refetchInterval: effectiveInterval,
  })
}

export function useTaskDetail(taskId: string) {
  const { effectiveInterval } = useAutoRefresh()
  return useQuery({
    queryKey: ['tasks', 'detail', taskId],
    queryFn: () =>
      apiClient<TaskDetailResponse>(
        `/v1/admin/tasks/${encodeURIComponent(taskId)}`,
      ),
    refetchInterval: effectiveInterval,
    enabled: !!taskId,
  })
}

export function useCancelTask() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: (taskId: string) =>
      apiClient<void>(
        `/v1/admin/tasks/${encodeURIComponent(taskId)}/cancel`,
        { method: 'POST' },
      ),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['tasks'] })
      toast.success('Task cancelled')
    },
    onError: (error: Error) => {
      toast.error('Failed to cancel task. ' + error.message)
    },
  })
}

// Utility functions

export function decodePayload(
  base64: string,
): { type: 'json'; data: unknown } | { type: 'binary'; raw: string } {
  if (!base64) {
    return { type: 'binary', raw: '' }
  }
  try {
    const decoded = atob(base64)
    const parsed = JSON.parse(decoded)
    return { type: 'json', data: parsed }
  } catch {
    return { type: 'binary', raw: base64 }
  }
}

export function canCancel(state: string): boolean {
  return state === 'pending' || state === 'running' || state === 'assigned'
}

export function taskStateLabel(state: string): string {
  if (!state) return ''
  return state.charAt(0).toUpperCase() + state.slice(1)
}
