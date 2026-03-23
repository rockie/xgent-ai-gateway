import { createFileRoute, redirect, Outlet } from '@tanstack/react-router'
import { SidebarProvider, SidebarInset } from '@/components/ui/sidebar'
import { AppSidebar } from '@/components/app-sidebar'
import { AppHeader } from '@/components/app-header'

export const Route = createFileRoute('/_authenticated')({
  beforeLoad: async ({ context, location }) => {
    if (!context.auth.isAuthenticated) {
      throw redirect({
        to: '/login',
        search: { redirect: location.href },
      })
    }
  },
  component: AuthenticatedLayout,
})

function getSidebarDefault() {
  const match = document.cookie.match(/(?:^|;\s*)sidebar_state=(\w+)/)
  return match ? match[1] === 'true' : true
}

function AuthenticatedLayout() {
  return (
    <SidebarProvider defaultOpen={getSidebarDefault()}>
      <AppSidebar />
      <SidebarInset>
        <AppHeader />
        <main className="p-6">
          <Outlet />
        </main>
      </SidebarInset>
    </SidebarProvider>
  )
}
