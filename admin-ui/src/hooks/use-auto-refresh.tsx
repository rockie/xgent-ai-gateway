import { createContext, useContext, useState, useMemo, useCallback, type ReactNode } from 'react'

type RefreshInterval = number | false

interface AutoRefreshContextValue {
  interval: RefreshInterval
  setInterval: (v: RefreshInterval) => void
  paused: boolean
  setPaused: (v: boolean) => void
  effectiveInterval: RefreshInterval
}

const AutoRefreshContext = createContext<AutoRefreshContextValue | null>(null)

export function AutoRefreshProvider({ children }: { children: ReactNode }) {
  const [interval, setIntervalState] = useState<RefreshInterval>(false)
  const [paused, setPaused] = useState(false)

  const effectiveInterval = useMemo(
    () => (paused ? false : interval),
    [paused, interval]
  )

  const setInterval = useCallback((v: RefreshInterval) => {
    setIntervalState(v)
    if (v === false) {
      setPaused(false)
    }
  }, [])

  return (
    <AutoRefreshContext.Provider
      value={{ interval, setInterval, paused, setPaused, effectiveInterval }}
    >
      {children}
    </AutoRefreshContext.Provider>
  )
}

export function useAutoRefresh() {
  const context = useContext(AutoRefreshContext)
  if (!context) {
    throw new Error('useAutoRefresh must be used within an AutoRefreshProvider')
  }
  return context
}
