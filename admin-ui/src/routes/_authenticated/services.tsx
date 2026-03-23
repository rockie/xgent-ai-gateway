import { createFileRoute } from '@tanstack/react-router'
import { EmptyState } from '@/components/empty-state'
import { Server } from 'lucide-react'

export const Route = createFileRoute('/_authenticated/services')({
  component: ServicesPage,
})

function ServicesPage() {
  return (
    <EmptyState
      icon={Server}
      heading="Coming Soon"
      description="This section is under development. Check back after the next update."
    />
  )
}
