import { useState } from 'react'
import { createFileRoute, Link } from '@tanstack/react-router'
import { Trash2 } from 'lucide-react'
import { Button } from '@/components/ui/button'
import {
  Card,
  CardContent,
  CardHeader,
  CardTitle,
} from '@/components/ui/card'
import {
  Breadcrumb,
  BreadcrumbItem,
  BreadcrumbLink,
  BreadcrumbList,
  BreadcrumbPage,
  BreadcrumbSeparator,
} from '@/components/ui/breadcrumb'
import { useServiceDetail, deriveServiceHealth } from '@/lib/services'
import { HealthBadge } from '@/components/health-badge'
import { NodeTable } from '@/components/node-table'
import { DeregisterDialog } from '@/components/deregister-dialog'
import { ErrorAlert } from '@/components/error-alert'
import { PageSkeleton } from '@/components/page-skeleton'

export const Route = createFileRoute('/_authenticated/services/$name')({
  component: ServiceDetailPage,
})

function ServiceDetailPage() {
  const { name } = Route.useParams()
  const { data, isLoading, isError, error, refetch } = useServiceDetail(name)
  const [deregisterOpen, setDeregisterOpen] = useState(false)

  return (
    <div className="space-y-6">
      <Breadcrumb>
        <BreadcrumbList>
          <BreadcrumbItem>
            <BreadcrumbLink render={<Link to="/services" />}>
              Services
            </BreadcrumbLink>
          </BreadcrumbItem>
          <BreadcrumbSeparator />
          <BreadcrumbItem>
            <BreadcrumbPage>{name}</BreadcrumbPage>
          </BreadcrumbItem>
        </BreadcrumbList>
      </Breadcrumb>

      {isLoading && <PageSkeleton lines={8} />}

      {isError && error && (
        <ErrorAlert message={error.message} onRetry={refetch} />
      )}

      {data && (
        <>
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-3">
              <h1 className="text-xl font-semibold">{name}</h1>
              <HealthBadge status={deriveServiceHealth(data.nodes)} />
            </div>
            <Button
              variant="destructive"
              size="sm"
              onClick={() => setDeregisterOpen(true)}
            >
              <Trash2 className="h-4 w-4 mr-2" />
              Deregister
            </Button>
          </div>

          <Card>
            <CardHeader>
              <CardTitle>Configuration</CardTitle>
            </CardHeader>
            <CardContent>
              <div className="grid grid-cols-2 gap-4">
                <ConfigItem
                  label="Description"
                  value={data.description || 'No description'}
                />
                <ConfigItem
                  label="Created"
                  value={new Date(data.created_at).toLocaleString()}
                />
                <ConfigItem
                  label="Task Timeout"
                  value={`${data.task_timeout_secs}s`}
                />
                <ConfigItem
                  label="Max Nodes"
                  value={`${data.max_nodes ?? 'Unlimited'}`}
                />
                <ConfigItem
                  label="Node Stale After"
                  value={`${data.node_stale_after_secs}s`}
                />
                <ConfigItem
                  label="Drain Timeout"
                  value={`${data.drain_timeout_secs}s`}
                />
              </div>
            </CardContent>
          </Card>

          <div className="space-y-4">
            <h2 className="text-lg font-semibold">
              Nodes ({data.nodes.length})
            </h2>
            <NodeTable nodes={data.nodes} />
          </div>

          <DeregisterDialog
            serviceName={name}
            open={deregisterOpen}
            onOpenChange={setDeregisterOpen}
          />
        </>
      )}
    </div>
  )
}

function ConfigItem({ label, value }: { label: string; value: string }) {
  return (
    <div>
      <dt className="text-sm text-muted-foreground">{label}</dt>
      <dd className="text-sm font-medium">{value}</dd>
    </div>
  )
}
