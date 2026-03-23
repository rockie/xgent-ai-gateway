import { createRootRouteWithContext, Outlet } from '@tanstack/react-router'
import type { QueryClient } from '@tanstack/react-query'
import { Toaster } from 'sonner'
import { ThemeProvider } from '@/hooks/use-theme'
import { AutoRefreshProvider } from '@/hooks/use-auto-refresh'
import { TooltipProvider } from '@/components/ui/tooltip'

interface RouterContext {
  queryClient: QueryClient
  auth: { isAuthenticated: boolean }
}

export const Route = createRootRouteWithContext<RouterContext>()({
  component: RootLayout,
})

function RootLayout() {
  return (
    <ThemeProvider>
      <AutoRefreshProvider>
        <TooltipProvider>
          <Outlet />
          <Toaster position="bottom-right" richColors />
        </TooltipProvider>
      </AutoRefreshProvider>
    </ThemeProvider>
  )
}
