import { useMemo } from 'react'
import { createFileRoute } from '@tanstack/react-router'
import { Activity, Server, Layers, ListTodo, LayoutDashboard } from 'lucide-react'
import { Card, CardContent, CardHeader } from '@/components/ui/card'
import { Skeleton } from '@/components/ui/skeleton'
import { EmptyState } from '@/components/empty-state'
import { ErrorAlert } from '@/components/error-alert'
import { OverviewCard } from '@/components/overview-card'
import { ThroughputChart } from '@/components/throughput-chart'
import { QueueDepthChart } from '@/components/queue-depth-chart'
import { ServiceHealthList } from '@/components/service-health-list'
import {
  useMetricsSummary,
  useMetricsHistory,
  computeRates,
  computeQueueDepthSeries,
  type HistoryPoint,
} from '@/lib/metrics'

export const Route = createFileRoute('/_authenticated/')({
  component: DashboardPage,
})

function computeTrend(
  current: number,
  previous: number | undefined,
  positive: 'up' | 'down',
): { delta: number; positive: 'up' | 'down' } | undefined {
  if (previous === undefined) return undefined
  const delta = current - previous
  if (delta === 0) return undefined
  return { delta, positive }
}

function formatRate(rate: number): string {
  return rate < 10 ? rate.toFixed(1) + '/min' : Math.round(rate) + '/min'
}

function sumValues(record: Record<string, number>): number {
  return Object.values(record).reduce((a, b) => a + b, 0)
}

function getPointNAgo(points: HistoryPoint[], n: number): HistoryPoint | undefined {
  if (points.length <= n) return undefined
  return points[points.length - 1 - n]
}

function DashboardPage() {
  const summaryQuery = useMetricsSummary()
  const historyQuery = useMetricsHistory()

  const summary = summaryQuery.data
  const history = historyQuery.data

  const rates = useMemo(
    () => computeRates(history?.points ?? []),
    [history],
  )

  const { data: queueData, services: queueServices } = useMemo(
    () => computeQueueDepthSeries(history?.points ?? []),
    [history],
  )

  // Delta trend computation: compare current vs ~5 minutes ago (30 entries at 10s interval)
  const points = history?.points ?? []
  const fiveMinAgo = getPointNAgo(points, 30)

  const nodesTrend = summary
    ? computeTrend(
        summary.active_nodes,
        fiveMinAgo ? sumValues(fiveMinAgo.nodes_active) : undefined,
        'up',
      )
    : undefined

  const queueTrend = summary
    ? computeTrend(
        summary.total_queue_depth,
        fiveMinAgo ? sumValues(fiveMinAgo.queue_depth) : undefined,
        'down',
      )
    : undefined

  // Throughput trend: compare current submitted rate vs 5 min ago rate
  const throughputTrend = useMemo(() => {
    if (!summary || !fiveMinAgo || points.length < 32) return undefined
    // Compute rate 5 minutes ago from the two points around that time
    const idx5min = points.length - 1 - 30
    if (idx5min < 1) return undefined
    const prev = points[idx5min - 1]
    const curr = points[idx5min]
    const dt = curr.timestamp - prev.timestamp
    if (dt <= 0) return undefined
    const oldRate = ((curr.tasks_submitted - prev.tasks_submitted) / dt) * 60
    const currentRate = summary.throughput.submitted_per_min
    const delta = Math.round(currentRate - oldRate)
    if (delta === 0) return undefined
    return { delta, positive: 'up' as const }
  }, [summary, fiveMinAgo, points])

  // Loading state
  if (summaryQuery.isLoading || historyQuery.isLoading) {
    return <DashboardSkeleton />
  }

  // Error state (show if both queries error)
  if (summaryQuery.isError && historyQuery.isError) {
    return (
      <ErrorAlert
        message={summaryQuery.error?.message ?? 'Failed to load metrics'}
        onRetry={() => {
          summaryQuery.refetch()
          historyQuery.refetch()
        }}
      />
    )
  }

  // Empty state (no services registered)
  if (summary && summary.service_count === 0) {
    return (
      <EmptyState
        icon={LayoutDashboard}
        heading="No Services Registered"
        description="Register a service to see dashboard metrics. Navigate to Services to get started."
      />
    )
  }

  // Data state
  return (
    <div className="space-y-6">
      {/* Overview Cards Row */}
      <div className="grid gap-4 grid-cols-4">
        <OverviewCard
          title="Services"
          value={summary?.service_count ?? 0}
          icon={Layers}
        />
        <OverviewCard
          title="Active Nodes"
          value={summary?.active_nodes ?? 0}
          icon={Server}
          trend={nodesTrend}
        />
        <OverviewCard
          title="Queue Depth"
          value={summary?.total_queue_depth ?? 0}
          icon={ListTodo}
          trend={queueTrend}
        />
        <OverviewCard
          title="Throughput"
          value={formatRate(summary?.throughput.submitted_per_min ?? 0)}
          icon={Activity}
          trend={throughputTrend}
        />
      </div>

      {/* Charts Row */}
      <div className="grid gap-4 grid-cols-2">
        <ThroughputChart data={rates} />
        <QueueDepthChart data={queueData} services={queueServices} />
      </div>

      {/* Service Health List */}
      <ServiceHealthList services={summary?.services ?? []} />
    </div>
  )
}

function DashboardSkeleton() {
  return (
    <div className="space-y-6">
      {/* Skeleton cards */}
      <div className="grid gap-4 grid-cols-4">
        {Array.from({ length: 4 }).map((_, i) => (
          <Card key={i}>
            <CardHeader className="pb-2">
              <Skeleton className="h-4 w-24" />
            </CardHeader>
            <CardContent>
              <Skeleton className="h-8 w-16" />
            </CardContent>
          </Card>
        ))}
      </div>

      {/* Skeleton charts */}
      <div className="grid gap-4 grid-cols-2">
        {Array.from({ length: 2 }).map((_, i) => (
          <Card key={i}>
            <CardHeader>
              <Skeleton className="h-5 w-32" />
            </CardHeader>
            <CardContent>
              <Skeleton className="h-[300px] w-full" />
            </CardContent>
          </Card>
        ))}
      </div>

      {/* Skeleton service list */}
      <div>
        <Skeleton className="h-6 w-32 mb-3" />
        <Card>
          <CardContent className="space-y-3 py-2">
            {Array.from({ length: 3 }).map((_, i) => (
              <Skeleton key={i} className="h-8 w-full" />
            ))}
          </CardContent>
        </Card>
      </div>
    </div>
  )
}
