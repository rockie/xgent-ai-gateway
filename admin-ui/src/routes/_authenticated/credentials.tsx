import { useState } from 'react'
import { createFileRoute } from '@tanstack/react-router'
import { useQueryClient } from '@tanstack/react-query'
import { KeyRound, Plus, ShieldCheck } from 'lucide-react'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { Button } from '@/components/ui/button'
import { Skeleton } from '@/components/ui/skeleton'
import {
  useApiKeys,
  useNodeTokens,
  useRevokeApiKey,
  useRevokeNodeToken,
  type ApiKeyListItem,
  type NodeTokenListItem,
} from '@/lib/credentials'
import { EmptyState } from '@/components/empty-state'
import { ErrorAlert } from '@/components/error-alert'
import { CredentialTable } from '@/components/credential-table'
import { CreateCredentialDialog } from '@/components/create-credential-dialog'
import { SecretRevealDialog } from '@/components/secret-reveal-dialog'
import { RevokeCredentialDialog } from '@/components/revoke-credential-dialog'

export const Route = createFileRoute('/_authenticated/credentials')({
  component: CredentialsPage,
})

function CredentialsPage() {
  const queryClient = useQueryClient()

  const [activeTab, setActiveTab] = useState<'api-keys' | 'node-tokens'>('api-keys')
  const [createDialogOpen, setCreateDialogOpen] = useState(false)
  const [revealSecret, setRevealSecret] = useState<{
    secret: string
    type: 'API key' | 'node token'
  } | null>(null)
  const [revokeTarget, setRevokeTarget] = useState<{
    item: ApiKeyListItem | NodeTokenListItem
    type: 'api-key' | 'node-token'
  } | null>(null)

  const apiKeys = useApiKeys()
  const nodeTokens = useNodeTokens()
  const revokeApiKey = useRevokeApiKey()
  const revokeNodeToken = useRevokeNodeToken()

  function handleCreated(secret: string) {
    setCreateDialogOpen(false)
    setRevealSecret({
      secret,
      type: activeTab === 'api-keys' ? 'API key' : 'node token',
    })
  }

  function handleRevealDismiss() {
    setRevealSecret(null)
    // Invalidate the relevant query cache after the secret reveal is dismissed
    if (activeTab === 'api-keys') {
      queryClient.invalidateQueries({ queryKey: ['api-keys'] })
    } else {
      queryClient.invalidateQueries({ queryKey: ['node-tokens'] })
    }
  }

  function handleRevokeClick(
    item: ApiKeyListItem | NodeTokenListItem,
    type: 'api-key' | 'node-token',
  ) {
    setRevokeTarget({ item, type })
  }

  function handleRevokeConfirm() {
    if (!revokeTarget) return

    if (revokeTarget.type === 'api-key') {
      const apiKeyItem = revokeTarget.item as ApiKeyListItem
      revokeApiKey.mutate(apiKeyItem.key_hash, {
        onSuccess: () => setRevokeTarget(null),
      })
    } else {
      const tokenItem = revokeTarget.item as NodeTokenListItem
      revokeNodeToken.mutate(
        {
          serviceName: tokenItem.service_name,
          tokenHash: tokenItem.token_hash,
        },
        {
          onSuccess: () => setRevokeTarget(null),
        },
      )
    }
  }

  const revokePending =
    revokeTarget?.type === 'api-key'
      ? revokeApiKey.isPending
      : revokeNodeToken.isPending

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <h2 className="text-xl font-semibold">Credentials</h2>
        <Button size="sm" onClick={() => setCreateDialogOpen(true)}>
          <Plus className="h-4 w-4 mr-1" />
          Create
        </Button>
      </div>

      <Tabs
        value={activeTab}
        onValueChange={(value) => setActiveTab(value as 'api-keys' | 'node-tokens')}
      >
        <TabsList>
          <TabsTrigger value="api-keys">API Keys</TabsTrigger>
          <TabsTrigger value="node-tokens">Node Tokens</TabsTrigger>
        </TabsList>

        <TabsContent value="api-keys" className="mt-4">
          <ApiKeysTabContent
            data={apiKeys.data?.api_keys}
            isLoading={apiKeys.isLoading}
            isError={apiKeys.isError}
            error={apiKeys.error}
            refetch={apiKeys.refetch}
            onRevoke={(item) => handleRevokeClick(item, 'api-key')}
          />
        </TabsContent>

        <TabsContent value="node-tokens" className="mt-4">
          <NodeTokensTabContent
            data={nodeTokens.data?.node_tokens}
            isLoading={nodeTokens.isLoading}
            isError={nodeTokens.isError}
            error={nodeTokens.error}
            refetch={nodeTokens.refetch}
            onRevoke={(item) => handleRevokeClick(item, 'node-token')}
          />
        </TabsContent>
      </Tabs>

      {/* Create credential dialog */}
      <CreateCredentialDialog
        open={createDialogOpen}
        onOpenChange={setCreateDialogOpen}
        credentialType={activeTab === 'api-keys' ? 'api-key' : 'node-token'}
        onCreated={handleCreated}
      />

      {/* Secret reveal dialog */}
      {revealSecret && (
        <SecretRevealDialog
          open={!!revealSecret}
          secret={revealSecret.secret}
          credentialType={revealSecret.type}
          onDismiss={handleRevealDismiss}
        />
      )}

      {/* Revoke confirmation dialog */}
      <RevokeCredentialDialog
        open={!!revokeTarget}
        onOpenChange={(open) => {
          if (!open) setRevokeTarget(null)
        }}
        onConfirm={handleRevokeConfirm}
        isPending={revokePending}
        credentialType={
          revokeTarget?.type === 'api-key' ? 'API key' : 'node token'
        }
      />
    </div>
  )
}

// --- Tab Content Components ---

function ApiKeysTabContent({
  data,
  isLoading,
  isError,
  error,
  refetch,
  onRevoke,
}: {
  data: ApiKeyListItem[] | undefined
  isLoading: boolean
  isError: boolean
  error: Error | null
  refetch: () => void
  onRevoke: (item: ApiKeyListItem) => void
}) {
  if (isLoading) return <TableSkeleton />
  if (isError && error) return <ErrorAlert message={error.message} onRetry={refetch} />
  if (!data || data.length === 0) {
    return (
      <EmptyState
        icon={KeyRound}
        heading="No API Keys"
        description="Create an API key to allow clients to submit tasks."
      />
    )
  }
  return (
    <CredentialTable
      type="api-key"
      data={data}
      onRevoke={(item) => onRevoke(item as ApiKeyListItem)}
    />
  )
}

function NodeTokensTabContent({
  data,
  isLoading,
  isError,
  error,
  refetch,
  onRevoke,
}: {
  data: NodeTokenListItem[] | undefined
  isLoading: boolean
  isError: boolean
  error: Error | null
  refetch: () => void
  onRevoke: (item: NodeTokenListItem) => void
}) {
  if (isLoading) return <TableSkeleton />
  if (isError && error) return <ErrorAlert message={error.message} onRetry={refetch} />
  if (!data || data.length === 0) {
    return (
      <EmptyState
        icon={ShieldCheck}
        heading="No Node Tokens"
        description="Create a node token to allow nodes to connect."
      />
    )
  }
  return (
    <CredentialTable
      type="node-token"
      data={data}
      onRevoke={(item) => onRevoke(item as NodeTokenListItem)}
    />
  )
}

function TableSkeleton() {
  return (
    <div className="space-y-3">
      <Skeleton className="h-10 w-full" />
      <Skeleton className="h-10 w-full" />
      <Skeleton className="h-10 w-full" />
    </div>
  )
}
