import { useQuery } from '@tanstack/react-query'
import { apiClient } from './api'
import { useAutoRefresh } from '@/hooks/use-auto-refresh'

// Types matching backend response shapes exactly

export interface MetricsSummaryResponse {
  service_count: number
  active_nodes: number
  total_queue_depth: number
  throughput: {
    submitted_per_min: number
    completed_per_min: number
  }
  services: ServiceHealthSummary[]
}

export interface ServiceHealthSummary {
  name: string
  health: string // "healthy" | "degraded" | "down" | "unknown"
  active_nodes: number
  total_nodes: number
  queue_depth: number
}

export interface HistoryPoint {
  timestamp: number
  tasks_submitted: number
  tasks_completed: number
  tasks_failed: number
  queue_depth: Record<string, number>
  nodes_active: Record<string, number>
}

export interface MetricsHistoryResponse {
  interval_secs: number
  points: HistoryPoint[]
}

// Hooks

export function useMetricsSummary() {
  const { effectiveInterval } = useAutoRefresh()
  return useQuery({
    queryKey: ['metrics', 'summary'],
    queryFn: () => apiClient<MetricsSummaryResponse>('/v1/admin/metrics/summary'),
    refetchInterval: effectiveInterval,
  })
}

export function useMetricsHistory() {
  const { effectiveInterval } = useAutoRefresh()
  return useQuery({
    queryKey: ['metrics', 'history'],
    queryFn: () => apiClient<MetricsHistoryResponse>('/v1/admin/metrics/history'),
    refetchInterval: effectiveInterval,
  })
}

// Chart data types

export interface ChartDataPoint {
  time: string
  submitted: number
  completed: number
}

export interface QueueDepthDataPoint {
  time: string
  [service: string]: string | number
}

// Utility functions

export function computeRates(points: HistoryPoint[]): ChartDataPoint[] {
  if (points.length < 2) return []
  return points.slice(1).map((point, i) => {
    const prev = points[i]
    const dtSecs = point.timestamp - prev.timestamp
    if (dtSecs <= 0) return { time: formatTime(point.timestamp), submitted: 0, completed: 0 }
    return {
      time: formatTime(point.timestamp),
      submitted: ((point.tasks_submitted - prev.tasks_submitted) / dtSecs) * 60,
      completed: ((point.tasks_completed - prev.tasks_completed) / dtSecs) * 60,
    }
  })
}

export function computeQueueDepthSeries(points: HistoryPoint[]): { data: QueueDepthDataPoint[]; services: string[] } {
  const serviceSet = new Set<string>()
  points.forEach(p => Object.keys(p.queue_depth).forEach(s => serviceSet.add(s)))
  const services = Array.from(serviceSet).sort()

  const data = points.map(point => {
    const entry: QueueDepthDataPoint = { time: formatTime(point.timestamp) }
    for (const svc of services) {
      entry[svc] = point.queue_depth[svc] ?? 0
    }
    return entry
  })

  return { data, services }
}

function formatTime(ts: number): string {
  const d = new Date(ts * 1000)
  return d.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit', second: '2-digit' })
}
