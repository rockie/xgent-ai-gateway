import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { useRouter } from '@tanstack/react-router'
import { apiClient } from './api'
import { toast } from 'sonner'

interface LoginRequest {
  username: string
  password: string
}

interface LoginResponse {
  username: string
}

export function useAuth() {
  return useQuery({
    queryKey: ['auth', 'session'],
    queryFn: () =>
      apiClient<LoginResponse>('/v1/admin/auth/refresh', { method: 'POST' }),
    retry: false,
    staleTime: 5 * 60 * 1000,
  })
}

export function useLogin() {
  return useMutation({
    mutationFn: (data: LoginRequest) =>
      apiClient<LoginResponse>('/v1/admin/auth/login', {
        method: 'POST',
        body: JSON.stringify(data),
      }),
  })
}

export function useLogout() {
  const queryClient = useQueryClient()
  const router = useRouter()
  return useMutation({
    mutationFn: () =>
      apiClient<void>('/v1/admin/auth/logout', { method: 'POST' }),
    onSuccess: () => {
      queryClient.clear()
      toast.success('Signed out successfully.')
      router.navigate({ to: '/login', search: { redirect: '/' } })
    },
    onError: () => {
      // Even on error, clear local state and redirect
      queryClient.clear()
      router.navigate({ to: '/login', search: { redirect: '/' } })
    },
  })
}
