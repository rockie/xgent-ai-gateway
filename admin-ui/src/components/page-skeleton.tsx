import { Skeleton } from '@/components/ui/skeleton'

interface PageSkeletonProps {
  lines?: number
  withHeader?: boolean
}

export function PageSkeleton({ lines = 5, withHeader = true }: PageSkeletonProps) {
  return (
    <div className="space-y-4">
      {withHeader && <Skeleton className="h-8 w-48" />}
      {Array.from({ length: lines }).map((_, i) => (
        <Skeleton key={i} className="h-4 w-full" />
      ))}
    </div>
  )
}
