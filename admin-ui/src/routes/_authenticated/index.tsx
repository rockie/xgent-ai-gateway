import { createFileRoute } from '@tanstack/react-router'
import { EmptyState } from '@/components/empty-state'
import { LayoutDashboard } from 'lucide-react'

export const Route = createFileRoute('/_authenticated/')({
  component: DashboardPage,
})

function DashboardPage() {
  return (
    <EmptyState
      icon={LayoutDashboard}
      heading="Coming Soon"
      description="This section is under development. Check back after the next update."
    />
  )
}
