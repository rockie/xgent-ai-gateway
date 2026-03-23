import { useState } from 'react'
import { Button } from '@/components/ui/button'
import { Copy, Check } from 'lucide-react'
import { decodePayload } from '@/lib/tasks'

interface JsonViewerProps {
  base64Data: string
  label?: string
}

export function JsonViewer({ base64Data, label }: JsonViewerProps) {
  const [copied, setCopied] = useState(false)

  if (!base64Data) {
    return (
      <p className="text-sm text-muted-foreground">No {label || 'data'} available</p>
    )
  }

  const decoded = decodePayload(base64Data)
  const displayText = decoded.type === 'json'
    ? JSON.stringify(decoded.data, null, 2)
    : decoded.raw

  const handleCopy = async () => {
    await navigator.clipboard.writeText(displayText)
    setCopied(true)
    setTimeout(() => setCopied(false), 2000)
  }

  return (
    <div className="relative">
      <div className="flex items-center justify-between mb-2">
        {decoded.type === 'binary' && (
          <span className="text-xs text-muted-foreground">Binary payload (base64)</span>
        )}
        <Button
          variant="ghost"
          size="icon"
          className="h-7 w-7 ml-auto"
          onClick={handleCopy}
        >
          {copied ? <Check className="h-3.5 w-3.5" /> : <Copy className="h-3.5 w-3.5" />}
        </Button>
      </div>
      <pre className="rounded-md bg-muted p-4 text-sm font-mono overflow-auto max-h-64 whitespace-pre-wrap break-all">
        {displayText}
      </pre>
    </div>
  )
}
