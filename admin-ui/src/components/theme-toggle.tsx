import { Moon, Sun } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { useTheme } from '@/hooks/use-theme'

export function ThemeToggle() {
  const { theme, toggleTheme } = useTheme()

  return (
    <Button variant="ghost" size="icon" onClick={toggleTheme}>
      {theme === 'dark' ? <Moon className="size-4" /> : <Sun className="size-4" />}
      <span className="sr-only">Toggle theme</span>
    </Button>
  )
}
