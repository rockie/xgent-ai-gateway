import { createFileRoute } from '@tanstack/react-router'
import { EmptyState } from '@/components/empty-state'
import { KeyRound } from 'lucide-react'

export const Route = createFileRoute('/_authenticated/credentials')({
  component: CredentialsPage,
})

function CredentialsPage() {
  return (
    <EmptyState
      icon={KeyRound}
      heading="Coming Soon"
      description="This section is under development. Check back after the next update."
    />
  )
}
