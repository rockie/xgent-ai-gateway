import { createFileRoute } from '@tanstack/react-router'
import { EmptyState } from '@/components/empty-state'
import { Server } from 'lucide-react'

export const Route = createFileRoute('/_authenticated/services/$name')({
  component: ServiceDetailPage,
})

function ServiceDetailPage() {
  const { name } = Route.useParams()
  return (
    <EmptyState
      icon={Server}
      heading={name}
      description="Service detail page will be available after the next update."
    />
  )
}
