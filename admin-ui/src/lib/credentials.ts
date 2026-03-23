import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { apiClient } from './api'
import { useAutoRefresh } from '@/hooks/use-auto-refresh'
import { toast } from 'sonner'

// Types matching backend response shapes exactly

export interface ApiKeyListItem {
  key_hash: string
  service_names: string[]
  label: string | null
  created_at: string
  expires_at: string | null
  callback_url: string | null
}

export interface ListApiKeysResponse {
  api_keys: ApiKeyListItem[]
}

export interface NodeTokenListItem {
  token_hash: string
  service_name: string
  label: string | null
  created_at: string
  expires_at: string | null
}

export interface ListNodeTokensResponse {
  node_tokens: NodeTokenListItem[]
}

export interface CreateApiKeyRequest {
  service_names: string[]
  callback_url?: string
  label?: string
  expires_at?: string // ISO 8601
}

export interface CreateApiKeyResponse {
  api_key: string // raw secret -- shown once
  key_hash: string
}

export interface CreateNodeTokenRequest {
  service_name: string
  node_label?: string
  expires_at?: string // ISO 8601
}

export interface CreateNodeTokenResponse {
  token: string // raw secret -- shown once
  token_hash: string
  service_name: string
}

// --- Query Hooks ---

export function useApiKeys() {
  const { effectiveInterval } = useAutoRefresh()
  return useQuery({
    queryKey: ['api-keys'],
    queryFn: () => apiClient<ListApiKeysResponse>('/v1/admin/api-keys'),
    refetchInterval: effectiveInterval,
  })
}

export function useNodeTokens() {
  const { effectiveInterval } = useAutoRefresh()
  return useQuery({
    queryKey: ['node-tokens'],
    queryFn: () => apiClient<ListNodeTokensResponse>('/v1/admin/node-tokens'),
    refetchInterval: effectiveInterval,
  })
}

// --- Mutation Hooks ---

// NOTE: Do NOT invalidate queries on create success.
// The secret reveal dialog needs the mutation response data.
// Invalidate AFTER the reveal dialog is dismissed.
export function useCreateApiKey() {
  return useMutation({
    mutationFn: (data: CreateApiKeyRequest) =>
      apiClient<CreateApiKeyResponse>('/v1/admin/api-keys', {
        method: 'POST',
        body: JSON.stringify(data),
      }),
    onError: (error: Error) => {
      toast.error('Failed to create API key. ' + error.message)
    },
  })
}

export function useCreateNodeToken() {
  return useMutation({
    mutationFn: (data: CreateNodeTokenRequest) =>
      apiClient<CreateNodeTokenResponse>('/v1/admin/node-tokens', {
        method: 'POST',
        body: JSON.stringify(data),
      }),
    onError: (error: Error) => {
      toast.error('Failed to create node token. ' + error.message)
    },
  })
}

// Optimistic revoke per D-06: row disappears immediately, reappears on error
export function useRevokeApiKey() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: (keyHash: string) =>
      apiClient<void>('/v1/admin/api-keys/revoke', {
        method: 'POST',
        body: JSON.stringify({ key_hash: keyHash }),
      }),
    onMutate: async (keyHash) => {
      await queryClient.cancelQueries({ queryKey: ['api-keys'] })
      const previous = queryClient.getQueryData<ListApiKeysResponse>(['api-keys'])
      queryClient.setQueryData<ListApiKeysResponse>(['api-keys'], (old) => {
        if (!old) return old
        return {
          ...old,
          api_keys: old.api_keys.filter((k) => k.key_hash !== keyHash),
        }
      })
      return { previous }
    },
    onError: (_err, _keyHash, context) => {
      if (context?.previous) {
        queryClient.setQueryData(['api-keys'], context.previous)
      }
      toast.error('Failed to revoke API key.')
    },
    onSuccess: () => {
      toast.success('API key revoked')
    },
    onSettled: () => {
      queryClient.invalidateQueries({ queryKey: ['api-keys'] })
    },
  })
}

export function useRevokeNodeToken() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: ({ serviceName, tokenHash }: { serviceName: string; tokenHash: string }) =>
      apiClient<void>('/v1/admin/node-tokens/revoke', {
        method: 'POST',
        body: JSON.stringify({ service_name: serviceName, token_hash: tokenHash }),
      }),
    onMutate: async ({ tokenHash }) => {
      await queryClient.cancelQueries({ queryKey: ['node-tokens'] })
      const previous = queryClient.getQueryData<ListNodeTokensResponse>(['node-tokens'])
      queryClient.setQueryData<ListNodeTokensResponse>(['node-tokens'], (old) => {
        if (!old) return old
        return {
          ...old,
          node_tokens: old.node_tokens.filter((t) => t.token_hash !== tokenHash),
        }
      })
      return { previous }
    },
    onError: (_err, _vars, context) => {
      if (context?.previous) {
        queryClient.setQueryData(['node-tokens'], context.previous)
      }
      toast.error('Failed to revoke node token.')
    },
    onSuccess: () => {
      toast.success('Node token revoked')
    },
    onSettled: () => {
      queryClient.invalidateQueries({ queryKey: ['node-tokens'] })
    },
  })
}

// --- Utility Functions ---

export function maskHash(hash: string): string {
  return hash.substring(0, 8) + '...'
}

export function isExpired(expiresAt: string | null): boolean {
  if (!expiresAt) return false
  return new Date(expiresAt) < new Date()
}
