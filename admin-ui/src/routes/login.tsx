import { createFileRoute } from '@tanstack/react-router'

export const Route = createFileRoute('/login')({
  validateSearch: (search: Record<string, unknown>) => ({
    redirect: (search.redirect as string) || '/',
  }),
  component: LoginPage,
})

function LoginPage() {
  return (
    <div className="flex items-center justify-center min-h-screen">
      <p>Login placeholder</p>
    </div>
  )
}
