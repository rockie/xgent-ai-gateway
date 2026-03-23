import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert'
import { Button } from '@/components/ui/button'
import { AlertCircle } from 'lucide-react'

interface ErrorAlertProps {
  message: string
  onRetry: () => void
}

export function ErrorAlert({ message, onRetry }: ErrorAlertProps) {
  return (
    <Alert variant="destructive">
      <AlertCircle className="h-4 w-4" />
      <AlertTitle>Something went wrong</AlertTitle>
      <AlertDescription className="mt-2">
        <p>{message}. Check your connection and try again.</p>
        <Button
          variant="outline"
          size="sm"
          onClick={onRetry}
          className="mt-3"
        >
          Retry request
        </Button>
      </AlertDescription>
    </Alert>
  )
}
