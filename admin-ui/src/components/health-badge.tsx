interface HealthBadgeProps {
  status: string
  draining?: boolean
}

function getHealthConfig(status: string, draining?: boolean) {
  if (draining) {
    return { color: 'bg-blue-500', label: 'Draining' }
  }

  switch (status) {
    case 'healthy':
      return { color: 'bg-green-500', label: 'Healthy' }
    case 'unhealthy':
    case 'stale':
      return { color: 'bg-yellow-500', label: 'Stale' }
    case 'degraded':
      return { color: 'bg-yellow-500', label: 'Degraded' }
    case 'disconnected':
      return { color: 'bg-red-500', label: 'Disconnected' }
    case 'down':
      return { color: 'bg-red-500', label: 'Down' }
    case 'unknown':
    default:
      return { color: 'bg-muted-foreground', label: 'Unknown' }
  }
}

export function HealthBadge({ status, draining }: HealthBadgeProps) {
  const { color, label } = getHealthConfig(status, draining)

  return (
    <span className="inline-flex items-center gap-1.5 text-sm">
      <span className={`h-2 w-2 rounded-full ${color}`} />
      {label}
    </span>
  )
}
