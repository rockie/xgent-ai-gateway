import { Construction, type LucideIcon } from 'lucide-react'

interface EmptyStateProps {
  icon?: LucideIcon
  heading: string
  description: string
}

export function EmptyState({
  icon: Icon = Construction,
  heading,
  description,
}: EmptyStateProps) {
  return (
    <div className="flex flex-col items-center justify-center py-16">
      <Icon className="h-12 w-12 text-muted-foreground mb-4" />
      <h2 className="text-[1.75rem] font-semibold leading-[1.2] text-foreground">
        {heading}
      </h2>
      <p className="mt-2 text-sm text-muted-foreground max-w-md text-center">
        {description}
      </p>
    </div>
  )
}
