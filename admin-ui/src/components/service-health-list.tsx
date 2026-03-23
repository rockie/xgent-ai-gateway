import { Link } from '@tanstack/react-router'
import { Card, CardContent } from '@/components/ui/card'
import type { ServiceHealthSummary } from '@/lib/metrics'

function getHealthDotColor(health: string): string {
  switch (health) {
    case 'healthy':
      return 'bg-green-500'
    case 'degraded':
      return 'bg-yellow-500'
    case 'down':
      return 'bg-red-500'
    case 'unknown':
    default:
      return 'bg-muted-foreground'
  }
}

interface ServiceHealthListProps {
  services: ServiceHealthSummary[]
}

export function ServiceHealthList({ services }: ServiceHealthListProps) {
  if (services.length === 0) return null

  return (
    <div>
      <h3 className="text-lg font-semibold mb-3">Service Health</h3>
      <Card>
        <CardContent className="divide-y">
          {services.map((svc) => (
            <Link
              key={svc.name}
              to="/services/$name"
              params={{ name: svc.name }}
              className="flex items-center justify-between py-2.5 px-2 -mx-2 no-underline hover:bg-accent rounded-md transition-colors"
            >
              <div className="flex items-center gap-2">
                <span
                  className={`h-2 w-2 rounded-full ${getHealthDotColor(svc.health)}`}
                />
                <span className="text-sm font-medium">{svc.name}</span>
              </div>
              <span className="text-sm text-muted-foreground">
                {svc.active_nodes}/{svc.total_nodes} nodes
              </span>
            </Link>
          ))}
        </CardContent>
      </Card>
    </div>
  )
}
