import { Link } from '@tanstack/react-router'
import {
  Card,
  CardHeader,
  CardTitle,
  CardDescription,
  CardContent,
  CardAction,
} from '@/components/ui/card'
import { Skeleton } from '@/components/ui/skeleton'
import { HealthBadge } from '@/components/health-badge'
import {
  type ServiceResponse,
  useServiceDetail,
  deriveServiceHealth,
} from '@/lib/services'

interface ServiceCardProps {
  service: ServiceResponse
}

export function ServiceCard({ service }: ServiceCardProps) {
  const detailQuery = useServiceDetail(service.name)

  const nodes = detailQuery.data?.nodes
  const activeNodes = nodes
    ? nodes.filter((n) => n.health === 'healthy' && !n.draining).length
    : 0
  const totalNodes = nodes ? nodes.length : 0
  const health = nodes ? deriveServiceHealth(nodes) : 'unknown'

  return (
    <Link
      to="/services/$name"
      params={{ name: service.name }}
      className="block no-underline"
    >
      <Card className="hover:border-primary/50 transition-colors cursor-pointer h-full">
        <CardHeader>
          <CardTitle className="text-base font-semibold">
            {service.name}
          </CardTitle>
          <CardAction>
            {detailQuery.isLoading ? (
              <Skeleton className="h-5 w-16" />
            ) : (
              <HealthBadge status={health} />
            )}
          </CardAction>
          {service.description && (
            <CardDescription className="text-sm text-muted-foreground line-clamp-2">
              {service.description}
            </CardDescription>
          )}
        </CardHeader>
        <CardContent>
          <div className="flex items-center justify-between text-sm text-muted-foreground">
            <span>
              {detailQuery.isLoading ? (
                <Skeleton className="h-4 w-20 inline-block" />
              ) : (
                `${activeNodes}/${totalNodes} nodes`
              )}
            </span>
            <span>{new Date(service.created_at).toLocaleDateString()}</span>
          </div>
        </CardContent>
      </Card>
    </Link>
  )
}
