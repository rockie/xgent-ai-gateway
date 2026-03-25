import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { apiClient } from './api'
import { useAutoRefresh } from '@/hooks/use-auto-refresh'
import { toast } from 'sonner'

// Types matching backend response shapes exactly

export interface ServiceResponse {
  name: string
  description: string
  created_at: string
  task_timeout_secs: number
  max_nodes: number | null
  node_stale_after_secs: number
  drain_timeout_secs: number
}

export interface NodeStatusResponse {
  node_id: string
  health: string
  last_seen: string
  in_flight_tasks: number
  draining: boolean
}

export interface ServiceDetailResponse extends ServiceResponse {
  nodes: NodeStatusResponse[]
}

export interface ListServicesResponse {
  services: ServiceResponse[]
}

export interface RegisterServiceRequest {
  name: string
  description?: string
  task_timeout_secs?: number
  max_nodes?: number
  node_stale_after_secs?: number
  drain_timeout_secs?: number
}

// Hooks

export function useServices() {
  const { effectiveInterval } = useAutoRefresh()
  return useQuery({
    queryKey: ['services'],
    queryFn: () => apiClient<ListServicesResponse>('/v1/admin/services'),
    refetchInterval: effectiveInterval,
  })
}

export function useServiceDetail(name: string) {
  const { effectiveInterval } = useAutoRefresh()
  return useQuery({
    queryKey: ['services', name],
    queryFn: () =>
      apiClient<ServiceDetailResponse>(
        `/v1/admin/services/${encodeURIComponent(name)}`,
      ),
    refetchInterval: effectiveInterval,
    enabled: !!name,
  })
}

export function useRegisterService() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: (data: RegisterServiceRequest) =>
      apiClient<ServiceResponse>('/v1/admin/services', {
        method: 'POST',
        body: JSON.stringify(data),
      }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['services'] })
      toast.success('Service registered successfully')
    },
    onError: (error: Error) => {
      toast.error('Failed to register service. ' + error.message)
    },
  })
}

export function useDeregisterService() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: (name: string) =>
      apiClient<void>(
        `/v1/admin/services/${encodeURIComponent(name)}`,
        { method: 'DELETE' },
      ),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['services'] })
      toast.success('Service deregistration started')
    },
    onError: (error: Error) => {
      toast.error('Failed to deregister service. ' + error.message)
    },
  })
}

// Utility functions

export function deriveServiceHealth(
  nodes: NodeStatusResponse[],
): 'healthy' | 'degraded' | 'down' | 'unknown' {
  if (nodes.length === 0) return 'unknown'
  const healthyCount = nodes.filter(
    (n) => n.health === 'healthy' && !n.draining,
  ).length
  if (healthyCount === nodes.length) return 'healthy'
  if (healthyCount === 0) return 'down'
  return 'degraded'
}

export function relativeTime(isoString: string): string {
  const diffMs = Date.now() - new Date(isoString).getTime()
  if (diffMs < 0) return 'just now'
  const seconds = Math.floor(diffMs / 1000)
  if (seconds < 60) return `${seconds}s ago`
  const minutes = Math.floor(seconds / 60)
  if (minutes < 60) return `${minutes}m ago`
  const hours = Math.floor(minutes / 60)
  if (hours < 24) return `${hours}h ago`
  return `${Math.floor(hours / 24)}d ago`
}
