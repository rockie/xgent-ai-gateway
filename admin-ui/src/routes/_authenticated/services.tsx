import { createFileRoute } from '@tanstack/react-router'

export const Route = createFileRoute('/_authenticated/services')({
  component: ServicesPage,
})

function ServicesPage() {
  return (
    <div className="flex items-center justify-center min-h-screen">
      <div className="text-center">
        <h1 className="text-xl font-semibold text-foreground">Coming Soon</h1>
        <p className="mt-2 text-sm text-muted-foreground">
          This section is under development. Check back after the next update.
        </p>
      </div>
    </div>
  )
}
