import { useState } from 'react'
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
  DialogFooter,
} from '@/components/ui/dialog'
import { Button } from '@/components/ui/button'
import { Copy, Check } from 'lucide-react'
import { toast } from 'sonner'

interface SecretRevealDialogProps {
  open: boolean
  secret: string
  credentialType: 'API key' | 'node token'
  onDismiss: () => void
}

export function SecretRevealDialog({
  open,
  secret,
  credentialType,
  onDismiss,
}: SecretRevealDialogProps) {
  const [copied, setCopied] = useState(false)

  const title =
    credentialType === 'API key'
      ? 'API Key Created'
      : 'Node Token Created'

  async function handleCopy() {
    await navigator.clipboard.writeText(secret)
    setCopied(true)
    toast.success('Copied to clipboard')
    setTimeout(() => setCopied(false), 2000)
  }

  function handleDismiss() {
    setCopied(false)
    onDismiss()
  }

  return (
    <Dialog
      open={open}
      onOpenChange={(nextOpen, details) => {
        // Block all dismiss attempts (escape key, outside press, close press)
        // Only allow closing via the "I've copied it" button
        if (!nextOpen) {
          const reason = details?.reason
          if (
            reason === 'escape-key' ||
            reason === 'outside-press' ||
            reason === 'close-press'
          ) {
            return
          }
        }
      }}
      disablePointerDismissal
    >
      <DialogContent showCloseButton={false} className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>{title}</DialogTitle>
          <DialogDescription>
            Save this secret now. You will not be able to see it again.
          </DialogDescription>
        </DialogHeader>

        <div className="rounded-md border border-amber-500/30 bg-amber-50 p-3 text-sm text-amber-800 dark:bg-amber-950/30 dark:text-amber-200">
          This secret will not be shown again.
        </div>

        <div className="flex items-center gap-2">
          <code className="flex-1 rounded-md border bg-muted p-3 text-xs font-mono break-all select-all">
            {secret}
          </code>
          <Button
            type="button"
            variant="outline"
            size="icon"
            onClick={handleCopy}
            aria-label="Copy to clipboard"
          >
            {copied ? (
              <Check className="h-4 w-4" />
            ) : (
              <Copy className="h-4 w-4" />
            )}
          </Button>
        </div>

        <DialogFooter>
          <Button onClick={handleDismiss}>
            I've copied it
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
