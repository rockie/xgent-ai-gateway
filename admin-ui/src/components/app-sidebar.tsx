import { Link, useRouterState } from '@tanstack/react-router'
import { LayoutDashboard, Server, ListTodo, KeyRound, User } from 'lucide-react'
import {
  Sidebar,
  SidebarContent,
  SidebarFooter,
  SidebarGroup,
  SidebarGroupContent,
  SidebarHeader,
  SidebarMenu,
  SidebarMenuButton,
  SidebarMenuItem,
  SidebarTrigger,
} from '@/components/ui/sidebar'

const navItems = [
  { label: 'Dashboard', icon: LayoutDashboard, to: '/' as const },
  { label: 'Services', icon: Server, to: '/services' as const },
  { label: 'Tasks', icon: ListTodo, to: '/tasks' as const },
  { label: 'Credentials', icon: KeyRound, to: '/credentials' as const },
]

export function AppSidebar() {
  const routerState = useRouterState()
  const currentPath = routerState.location.pathname

  return (
    <Sidebar collapsible="icon">
      <SidebarHeader>
        <div className="flex h-8 items-center px-2 font-semibold text-sm tracking-tight group-data-[collapsible=icon]:hidden">
          xgent
        </div>
      </SidebarHeader>
      <SidebarContent>
        <SidebarGroup>
          <SidebarGroupContent>
            <SidebarMenu>
              {navItems.map((item) => {
                const isActive =
                  item.to === '/'
                    ? currentPath === '/'
                    : currentPath.startsWith(item.to)
                return (
                  <SidebarMenuItem key={item.label}>
                    <SidebarMenuButton
                      isActive={isActive}
                      tooltip={item.label}
                      render={<Link to={item.to} />}
                    >
                      <item.icon />
                      <span>{item.label}</span>
                    </SidebarMenuButton>
                  </SidebarMenuItem>
                )
              })}
            </SidebarMenu>
          </SidebarGroupContent>
        </SidebarGroup>
      </SidebarContent>
      <SidebarFooter>
        <div className="flex items-center justify-between px-2">
          <div className="flex items-center gap-2 group-data-[collapsible=icon]:hidden">
            <User className="size-4 text-muted-foreground" />
            <span className="text-sm text-muted-foreground">Admin</span>
          </div>
          <SidebarTrigger />
        </div>
      </SidebarFooter>
    </Sidebar>
  )
}
