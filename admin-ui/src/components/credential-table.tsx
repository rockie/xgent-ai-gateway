import { Trash2 } from 'lucide-react'
import { Button } from '@/components/ui/button'
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from '@/components/ui/table'
import {
  maskHash,
  isExpired,
  type ApiKeyListItem,
  type NodeTokenListItem,
} from '@/lib/credentials'

interface CredentialTableProps {
  type: 'api-key' | 'node-token'
  data: ApiKeyListItem[] | NodeTokenListItem[]
  onRevoke: (item: ApiKeyListItem | NodeTokenListItem) => void
}

export function CredentialTable({ type, data, onRevoke }: CredentialTableProps) {
  const isApiKey = type === 'api-key'

  return (
    <Table>
      <TableHeader>
        <TableRow>
          <TableHead>{isApiKey ? 'Key Hash' : 'Token Hash'}</TableHead>
          <TableHead>{isApiKey ? 'Services' : 'Service'}</TableHead>
          <TableHead>Label</TableHead>
          <TableHead>Created</TableHead>
          <TableHead>Expiry</TableHead>
          <TableHead className="w-[80px]" />
        </TableRow>
      </TableHeader>
      <TableBody>
        {data.map((item) => {
          const hash = isApiKey
            ? (item as ApiKeyListItem).key_hash
            : (item as NodeTokenListItem).token_hash
          const service = isApiKey
            ? (item as ApiKeyListItem).service_names.join(', ')
            : (item as NodeTokenListItem).service_name
          const expired = isExpired(item.expires_at)

          return (
            <TableRow key={hash}>
              <TableCell className="font-mono text-xs">
                {maskHash(hash)}
              </TableCell>
              <TableCell>{service}</TableCell>
              <TableCell>{item.label || '\u2014'}</TableCell>
              <TableCell>
                {new Date(item.created_at).toLocaleDateString()}
              </TableCell>
              <TableCell>
                {item.expires_at ? (
                  <span className={expired ? 'text-destructive' : ''}>
                    {new Date(item.expires_at).toLocaleDateString()}
                    {expired && ' (expired)'}
                  </span>
                ) : (
                  '\u2014'
                )}
              </TableCell>
              <TableCell>
                <Button
                  variant="ghost"
                  size="icon-sm"
                  onClick={() => onRevoke(item)}
                  aria-label="Revoke"
                >
                  <Trash2 className="h-4 w-4 text-destructive" />
                </Button>
              </TableCell>
            </TableRow>
          )
        })}
      </TableBody>
    </Table>
  )
}
