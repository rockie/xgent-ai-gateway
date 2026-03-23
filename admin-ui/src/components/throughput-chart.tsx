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
import type { ChartDataPoint } from '@/lib/metrics'

interface ThroughputChartProps {
  data: ChartDataPoint[]
}

export function ThroughputChart({ data }: ThroughputChartProps) {
  return (
    <Card>
      <CardHeader>
        <CardTitle>Task Throughput</CardTitle>
      </CardHeader>
      <CardContent>
        <div className="h-[300px]">
          {data.length === 0 ? (
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
                <Area
                  type="monotone"
                  dataKey="submitted"
                  name="Submitted"
                  stackId="1"
                  stroke="#3b82f6"
                  fill="#3b82f6"
                  fillOpacity={0.3}
                />
                <Area
                  type="monotone"
                  dataKey="completed"
                  name="Completed"
                  stackId="1"
                  stroke="#22c55e"
                  fill="#22c55e"
                  fillOpacity={0.3}
                />
              </AreaChart>
            </ResponsiveContainer>
          )}
        </div>
      </CardContent>
    </Card>
  )
}
