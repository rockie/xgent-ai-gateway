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
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Checkbox } from '@/components/ui/checkbox'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from '@/components/ui/popover'
import { Loader2 } from 'lucide-react'
import { useServices } from '@/lib/services'
import { useCreateApiKey, useCreateNodeToken } from '@/lib/credentials'

interface CreateCredentialDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  credentialType: 'api-key' | 'node-token'
  onCreated: (secret: string) => void
}

export function CreateCredentialDialog({
  open,
  onOpenChange,
  credentialType,
  onCreated,
}: CreateCredentialDialogProps) {
  const { data: servicesData } = useServices()
  const createApiKey = useCreateApiKey()
  const createNodeToken = useCreateNodeToken()

  // Form state
  const [selectedServices, setSelectedServices] = useState<Set<string>>(new Set())
  const [selectedService, setSelectedService] = useState<string>('')
  const [label, setLabel] = useState('')
  const [expiryDate, setExpiryDate] = useState('')
  const [callbackUrl, setCallbackUrl] = useState('')

  const isPending =
    credentialType === 'api-key'
      ? createApiKey.isPending
      : createNodeToken.isPending

  const isApiKey = credentialType === 'api-key'
  const title = isApiKey ? 'Create API Key' : 'Create Node Token'

  function resetForm() {
    setSelectedServices(new Set())
    setSelectedService('')
    setLabel('')
    setExpiryDate('')
    setCallbackUrl('')
  }

  function handleServiceToggle(name: string, checked: boolean) {
    setSelectedServices((prev) => {
      const next = new Set(prev)
      if (checked) {
        next.add(name)
      } else {
        next.delete(name)
      }
      return next
    })
  }

  function handleSubmit(e: React.FormEvent) {
    e.preventDefault()

    if (isApiKey) {
      const serviceNames = Array.from(selectedServices)
      if (serviceNames.length === 0) return

      createApiKey.mutate(
        {
          service_names: serviceNames,
          label: label.trim() || undefined,
          expires_at: expiryDate
            ? new Date(expiryDate + 'T23:59:59Z').toISOString()
            : undefined,
          callback_url: callbackUrl.trim() || undefined,
        },
        {
          onSuccess: (data) => {
            resetForm()
            onCreated(data.api_key)
          },
        },
      )
    } else {
      if (!selectedService) return

      createNodeToken.mutate(
        {
          service_name: selectedService,
          node_label: label.trim() || undefined,
          expires_at: expiryDate
            ? new Date(expiryDate + 'T23:59:59Z').toISOString()
            : undefined,
        },
        {
          onSuccess: (data) => {
            resetForm()
            onCreated(data.token)
          },
        },
      )
    }
  }

  const canSubmit = isApiKey
    ? selectedServices.size > 0
    : !!selectedService

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>{title}</DialogTitle>
          <DialogDescription>
            {isApiKey
              ? 'Create an API key for client access to selected services.'
              : 'Create a node token for a compute node to connect to a service.'}
          </DialogDescription>
        </DialogHeader>
        <form onSubmit={handleSubmit}>
          <div className="grid gap-4 py-4">
            {/* Service selection */}
            {isApiKey ? (
              <div className="grid gap-2">
                <Label>Services</Label>
                <Popover>
                  <PopoverTrigger
                    render={
                      <Button
                        variant="outline"
                        className="justify-start font-normal"
                        type="button"
                      />
                    }
                  >
                    {selectedServices.size > 0
                      ? `${selectedServices.size} service${selectedServices.size > 1 ? 's' : ''} selected`
                      : 'Select services...'}
                  </PopoverTrigger>
                  <PopoverContent className="w-56 p-3" align="start">
                    <div className="space-y-2">
                      {servicesData?.services.map((svc) => (
                        <label
                          key={svc.name}
                          className="flex items-center gap-2 cursor-pointer"
                        >
                          <Checkbox
                            checked={selectedServices.has(svc.name)}
                            onCheckedChange={(checked) =>
                              handleServiceToggle(svc.name, checked)
                            }
                          />
                          <span className="text-sm">{svc.name}</span>
                        </label>
                      ))}
                      {(!servicesData || servicesData.services.length === 0) && (
                        <p className="text-sm text-muted-foreground">
                          No services registered.
                        </p>
                      )}
                    </div>
                  </PopoverContent>
                </Popover>
              </div>
            ) : (
              <div className="grid gap-2">
                <Label>Service</Label>
                <Select
                  value={selectedService}
                  onValueChange={(value) => setSelectedService(value ?? '')}
                >
                  <SelectTrigger className="w-full">
                    <SelectValue placeholder="Select a service..." />
                  </SelectTrigger>
                  <SelectContent>
                    {servicesData?.services.map((svc) => (
                      <SelectItem key={svc.name} value={svc.name}>
                        {svc.name}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </div>
            )}

            {/* Label */}
            <div className="grid gap-2">
              <Label htmlFor="credential-label">Label</Label>
              <Input
                id="credential-label"
                placeholder="e.g., Production key"
                value={label}
                onChange={(e) => setLabel(e.target.value)}
              />
            </div>

            {/* Expiry date */}
            <div className="grid gap-2">
              <Label htmlFor="credential-expiry">Expiry Date</Label>
              <input
                id="credential-expiry"
                type="date"
                className="flex h-8 w-full rounded-lg border border-input bg-transparent px-2.5 py-2 text-sm outline-none focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/50 dark:bg-input/30"
                value={expiryDate}
                onChange={(e) => setExpiryDate(e.target.value)}
                min={new Date().toISOString().split('T')[0]}
              />
            </div>

            {/* Callback URL (API keys only) */}
            {isApiKey && (
              <div className="grid gap-2">
                <Label htmlFor="credential-callback">Callback URL</Label>
                <Input
                  id="credential-callback"
                  type="url"
                  placeholder="https://example.com/callback"
                  value={callbackUrl}
                  onChange={(e) => setCallbackUrl(e.target.value)}
                />
              </div>
            )}
          </div>
          <DialogFooter>
            <Button
              type="button"
              variant="outline"
              onClick={() => onOpenChange(false)}
            >
              Cancel
            </Button>
            <Button type="submit" disabled={isPending || !canSubmit}>
              {isPending && <Loader2 className="mr-2 h-4 w-4 animate-spin" />}
              {isPending
                ? 'Creating...'
                : isApiKey
                  ? 'Create API Key'
                  : 'Create Node Token'}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  )
}
