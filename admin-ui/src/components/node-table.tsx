import { Server } from 'lucide-react'
import type { NodeStatusResponse } from '@/lib/services'
import { relativeTime } from '@/lib/services'
import { HealthBadge } from '@/components/health-badge'
import { EmptyState } from '@/components/empty-state'
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from '@/components/ui/table'

interface NodeTableProps {
  nodes: NodeStatusResponse[]
}

export function NodeTable({ nodes }: NodeTableProps) {
  if (nodes.length === 0) {
    return (
      <EmptyState
        icon={Server}
        heading="No nodes connected"
        description="Nodes will appear here once they connect to this service."
      />
    )
  }

  return (
    <Table>
      <TableHeader>
        <TableRow>
          <TableHead>Node ID</TableHead>
          <TableHead>Health</TableHead>
          <TableHead>In-flight Tasks</TableHead>
          <TableHead>Last Seen</TableHead>
        </TableRow>
      </TableHeader>
      <TableBody>
        {nodes.map((node) => (
          <TableRow key={node.node_id}>
            <TableCell>
              <span
                className="font-mono text-sm truncate max-w-[200px] inline-block"
                title={node.node_id}
              >
                {node.node_id}
              </span>
            </TableCell>
            <TableCell>
              <HealthBadge status={node.health} draining={node.draining} />
            </TableCell>
            <TableCell>{node.in_flight_tasks}</TableCell>
            <TableCell title={node.last_seen}>
              {relativeTime(node.last_seen)}
            </TableCell>
          </TableRow>
        ))}
      </TableBody>
    </Table>
  )
}
