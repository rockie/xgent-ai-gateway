import { createFileRoute } from '@tanstack/react-router'
import { useState } from 'react'
import { Server, Plus } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { useServices } from '@/lib/services'
import { ServiceCard } from '@/components/service-card'
import { ServiceRegistrationDialog } from '@/components/service-registration-dialog'
import { EmptyState } from '@/components/empty-state'
import { ErrorAlert } from '@/components/error-alert'
import { PageSkeleton } from '@/components/page-skeleton'

export const Route = createFileRoute('/_authenticated/services')({
  component: ServicesPage,
})

function ServicesPage() {
  const { data, isLoading, isError, error, refetch } = useServices()
  const [registerOpen, setRegisterOpen] = useState(false)

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <h1 className="text-xl font-semibold">Services</h1>
        <Button onClick={() => setRegisterOpen(true)}>
          <Plus className="h-4 w-4 mr-2" />
          Register Service
        </Button>
      </div>

      {isLoading && <PageSkeleton lines={6} />}

      {isError && error && (
        <ErrorAlert message={error.message} onRetry={refetch} />
      )}

      {data && data.services.length === 0 && (
        <EmptyState
          icon={Server}
          heading="No services registered"
          description="Register your first service to get started."
          action={
            <Button onClick={() => setRegisterOpen(true)}>
              <Plus className="h-4 w-4 mr-2" />
              Register your first service
            </Button>
          }
        />
      )}

      {data && data.services.length > 0 && (
        <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
          {data.services.map((svc) => (
            <ServiceCard key={svc.name} service={svc} />
          ))}
        </div>
      )}

      <ServiceRegistrationDialog
        open={registerOpen}
        onOpenChange={setRegisterOpen}
      />
    </div>
  )
}
