import { Card, CardHeader, CardTitle, CardContent } from '@/components/ui/card'
import { ArrowUp, ArrowDown, type LucideIcon } from 'lucide-react'

interface TrendProps {
  delta: number
  positive: 'up' | 'down'
}

interface OverviewCardProps {
  title: string
  value: string | number
  icon: LucideIcon
  trend?: TrendProps
}

function TrendIndicator({ delta, positive }: TrendProps) {
  const isUp = delta > 0
  const isGood =
    (isUp && positive === 'up') || (!isUp && positive === 'down')

  const color = isGood ? 'text-green-500' : 'text-red-500'
  const Icon = isUp ? ArrowUp : ArrowDown
  const sign = isUp ? '+' : ''

  return (
    <div className={`flex items-center gap-0.5 text-xs ${color}`}>
      <Icon className="h-3 w-3" />
      <span>
        {sign}
        {delta}
      </span>
    </div>
  )
}

export function OverviewCard({ title, value, icon: Icon, trend }: OverviewCardProps) {
  return (
    <Card>
      <CardHeader className="flex flex-row items-center justify-between pb-2">
        <CardTitle className="text-sm font-medium text-muted-foreground">
          {title}
        </CardTitle>
        <Icon className="h-4 w-4 text-muted-foreground" />
      </CardHeader>
      <CardContent>
        <div className="text-2xl font-bold">{value}</div>
        {trend && <TrendIndicator {...trend} />}
      </CardContent>
    </Card>
  )
}
