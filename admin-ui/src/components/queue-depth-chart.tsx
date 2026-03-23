import {
  AreaChart,
  Area,
  XAxis,
  YAxis,
  CartesianGrid,
  Tooltip,
  ResponsiveContainer,
} from 'recharts'
import { Card, CardHeader, CardTitle, CardContent } from '@/components/ui/card'
import type { QueueDepthDataPoint } from '@/lib/metrics'

const COLORS = [
  '#3b82f6',
  '#22c55e',
  '#f59e0b',
  '#ef4444',
  '#8b5cf6',
  '#06b6d4',
  '#ec4899',
]

interface QueueDepthChartProps {
  data: QueueDepthDataPoint[]
  services: string[]
}

export function QueueDepthChart({ data, services }: QueueDepthChartProps) {
  const isEmpty = data.length === 0 || services.length === 0

  return (
    <Card>
      <CardHeader>
        <CardTitle>Queue Depth</CardTitle>
      </CardHeader>
      <CardContent>
        <div className="h-[300px]">
          {isEmpty ? (
            <div className="flex h-full items-center justify-center text-sm text-muted-foreground">
              Collecting data...
            </div>
          ) : (
            <ResponsiveContainer width="100%" height="100%">
              <AreaChart data={data}>
                <CartesianGrid strokeDasharray="3 3" />
                <XAxis dataKey="time" tick={{ fontSize: 12 }} />
                <YAxis tick={{ fontSize: 12 }} />
                <Tooltip />
                {services.map((svc, i) => (
                  <Area
                    key={svc}
                    type="monotone"
                    dataKey={svc}
                    name={svc}
                    stackId="1"
                    stroke={COLORS[i % COLORS.length]}
                    fill={COLORS[i % COLORS.length]}
                    fillOpacity={0.3}
                  />
                ))}
              </AreaChart>
            </ResponsiveContainer>
          )}
        </div>
      </CardContent>
    </Card>
  )
}
