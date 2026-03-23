import { createFileRoute, useNavigate, useRouter, useSearch } from '@tanstack/react-router'
import { useState } from 'react'
import { useQueryClient } from '@tanstack/react-query'
import { useLogin } from '@/lib/auth'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from '@/components/ui/card'
import { Alert, AlertDescription } from '@/components/ui/alert'
import { AlertCircle, Loader2 } from 'lucide-react'
import { toast } from 'sonner'

export const Route = createFileRoute('/login')({
  validateSearch: (search: Record<string, unknown>) => ({
    redirect: (search.redirect as string) || '/',
  }),
  component: LoginPage,
})

function LoginPage() {
  const navigate = useNavigate()
  const router = useRouter()
  const { redirect } = useSearch({ from: '/login' })
  const queryClient = useQueryClient()
  const login = useLogin()
  const [username, setUsername] = useState('')
  const [password, setPassword] = useState('')

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault()
    login.mutate(
      { username, password },
      {
        onSuccess: async () => {
          toast.success('Signed in successfully.')
          await queryClient.invalidateQueries({ queryKey: ['auth'] })
          // Update router context directly so beforeLoad sees isAuthenticated
          router.update({
            context: {
              ...router.options.context,
              auth: { isAuthenticated: true },
            },
          })
          await router.invalidate()
          navigate({ to: redirect || '/' })
        },
      },
    )
  }

  const errorMessage = login.error
    ? login.error.name === 'AuthError' || (login.error as unknown as { status: number })?.status === 401
      ? 'Invalid username or password. Check your credentials and try again.'
      : 'Unable to reach the gateway. Verify the server is running and try again.'
    : null

  return (
    <div className="flex min-h-screen">
      {/* Brand panel -- hidden below 1280px */}
      <div className="hidden min-[1280px]:flex min-[1280px]:w-1/2 items-center justify-center bg-card">
        <div className="text-center">
          <h1 className="text-[1.75rem] font-semibold leading-[1.2] text-foreground">
            xgent gateway
          </h1>
          <p className="mt-2 text-sm text-muted-foreground">
            Pull-model task gateway for internal compute nodes
          </p>
        </div>
      </div>

      {/* Login form panel */}
      <div className="flex w-full min-[1280px]:w-1/2 items-center justify-center bg-background p-8">
        <Card className="w-full max-w-[24rem]">
          <CardHeader>
            <CardTitle className="text-[1.25rem] font-semibold leading-[1.2]">
              Sign in to gateway
            </CardTitle>
            <CardDescription className="text-sm">
              Enter your credentials to access the admin panel.
            </CardDescription>
          </CardHeader>
          <CardContent>
            <form onSubmit={handleSubmit} className="space-y-4">
              <div className="space-y-2">
                <label htmlFor="username" className="text-xs text-muted-foreground">
                  Username
                </label>
                <Input
                  id="username"
                  type="text"
                  placeholder="admin"
                  value={username}
                  onChange={(e) => setUsername(e.target.value)}
                  disabled={login.isPending}
                  required
                />
              </div>
              <div className="space-y-2">
                <label htmlFor="password" className="text-xs text-muted-foreground">
                  Password
                </label>
                <Input
                  id="password"
                  type="password"
                  value={password}
                  onChange={(e) => setPassword(e.target.value)}
                  disabled={login.isPending}
                  required
                />
              </div>
              <Button type="submit" className="w-full" disabled={login.isPending}>
                {login.isPending ? (
                  <>
                    <Loader2 className="mr-2 h-4 w-4 animate-spin" />
                    Signing in...
                  </>
                ) : (
                  'Sign in to gateway'
                )}
              </Button>
            </form>
            {errorMessage && (
              <Alert variant="destructive" className="mt-4">
                <AlertCircle className="h-4 w-4" />
                <AlertDescription>{errorMessage}</AlertDescription>
              </Alert>
            )}
          </CardContent>
        </Card>
      </div>
    </div>
  )
}
