import { Badge } from '@/components/ui/badge'
import { taskStateLabel } from '@/lib/tasks'

const stateColors: Record<string, string> = {
  pending: 'bg-yellow-500/20 text-yellow-500 border-yellow-500/30',
  assigned: 'bg-blue-500/20 text-blue-500 border-blue-500/30',
  running: 'bg-blue-500/20 text-blue-500 border-blue-500/30',
  completed: 'bg-green-500/20 text-green-500 border-green-500/30',
  failed: 'bg-red-500/20 text-red-500 border-red-500/30',
}

export function TaskStatusBadge({ state }: { state: string }) {
  const colorClasses = stateColors[state] || stateColors.pending
  return (
    <Badge variant="outline" className={colorClasses}>
      {taskStateLabel(state)}
    </Badge>
  )
}
