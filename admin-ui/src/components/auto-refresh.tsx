import { RefreshCw } from 'lucide-react'
import { Button } from '@/components/ui/button'
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'
import { useAutoRefresh } from '@/hooks/use-auto-refresh'

const INTERVAL_OPTIONS = [
  { label: 'Off', value: false as const },
  { label: '5s', value: 5000 },
  { label: '15s', value: 15000 },
  { label: '30s', value: 30000 },
]

export function AutoRefresh() {
  const { interval, setInterval, paused, setPaused } = useAutoRefresh()

  const isActive = interval !== false && !paused
  const displayText = interval === false
    ? 'Off'
    : paused
      ? `${interval / 1000}s (Paused)`
      : `${interval / 1000}s`

  return (
    <DropdownMenu>
      <DropdownMenuTrigger
        render={
          <Button variant="ghost" size="sm" className="gap-1.5 text-xs">
            <RefreshCw className={`size-3.5 ${isActive ? 'animate-spin' : ''}`} />
            <span>{displayText}</span>
          </Button>
        }
      />
      <DropdownMenuContent align="end">
        {INTERVAL_OPTIONS.map((option) => (
          <DropdownMenuItem
            key={String(option.value)}
            onClick={() => setInterval(option.value)}
          >
            {option.label}
          </DropdownMenuItem>
        ))}
        {interval !== false && (
          <>
            <DropdownMenuSeparator />
            <DropdownMenuItem onClick={() => setPaused(!paused)}>
              {paused ? 'Resume' : 'Pause'}
            </DropdownMenuItem>
          </>
        )}
      </DropdownMenuContent>
    </DropdownMenu>
  )
}
