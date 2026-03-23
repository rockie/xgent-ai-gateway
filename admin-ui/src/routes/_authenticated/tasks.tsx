import { createFileRoute } from '@tanstack/react-router'
import { EmptyState } from '@/components/empty-state'
import { ListTodo } from 'lucide-react'

export const Route = createFileRoute('/_authenticated/tasks')({
  component: TasksPage,
})

function TasksPage() {
  return (
    <EmptyState
      icon={ListTodo}
      heading="Coming Soon"
      description="This section is under development. Check back after the next update."
    />
  )
}
