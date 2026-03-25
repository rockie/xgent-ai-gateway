---
status: awaiting_human_verify
trigger: "Admin UI cannot open service detail page — clicking a service card does nothing or shows blank content."
created: 2026-03-25T00:00:00Z
updated: 2026-03-25T00:00:00Z
---

## Current Focus

hypothesis: CONFIRMED — services.tsx rendered full list content without Outlet, blocking child route rendering
test: Build and type-check after fix
expecting: Clean build with both services.index and services.$name as separate chunks
next_action: Await human verification that clicking a service card navigates to the detail page

## Symptoms

expected: Clicking a service card in /services navigates to /services/$name and shows service detail (config, nodes, deregister button)
actual: Service detail page cannot render — the child route services.$name.tsx has no Outlet in its parent services.tsx to mount into
errors: No explicit errors — the page just doesn't show the detail view
reproduction: Navigate to /services, click any service card
started: Since the routing was set up — services.tsx renders list content directly without an Outlet

## Eliminated

(none — root cause identified on first hypothesis)

## Evidence

- timestamp: 2026-03-25T00:00:00Z
  checked: admin-ui/src/routes/_authenticated/services.tsx
  found: File renders full ServicesPage content (list, empty state, registration dialog) with no Outlet component
  implication: Child route services.$name.tsx has no mount point — TanStack Router requires parent layout routes to render Outlet for children

- timestamp: 2026-03-25T00:00:00Z
  checked: admin-ui/src/routeTree.gen.ts line 54
  found: AuthenticatedServicesNameRoute has getParentRoute => AuthenticatedServicesRoute, confirming services.$name is a child of services
  implication: The parent-child relationship is correctly wired in the route tree; the problem is the missing Outlet in the parent component

- timestamp: 2026-03-25T00:00:00Z
  checked: admin-ui/src/routes/_authenticated.tsx
  found: _authenticated layout correctly uses Outlet pattern — serves as reference for the fix
  implication: The Outlet pattern works elsewhere in this codebase; services.tsx just needs the same treatment

- timestamp: 2026-03-25T00:00:00Z
  checked: admin-ui/src/components/service-card.tsx
  found: ServiceCard uses Link to="/services/$name" with params={{ name: service.name }} — navigation is correctly wired
  implication: Navigation triggers correctly; the child route simply has no place to render

- timestamp: 2026-03-25T00:00:00Z
  checked: TypeScript compilation and Vite build after fix
  found: tsc -b --noEmit passes with zero errors; vite build succeeds in 426ms with services.index and services.$name as separate code-split chunks
  implication: Fix is structurally correct — no type errors or import issues

## Resolution

root_cause: In TanStack Router file-based routing, services.tsx is the parent layout for services.$name.tsx. But services.tsx rendered the full services list directly without an <Outlet />. The child route (service detail page) had nowhere to mount.
fix: Split services.tsx into (1) a thin layout wrapper that renders only <Outlet />, and (2) services.index.tsx containing the services list content. Regenerated the route tree.
verification: TypeScript type-check passes. Vite build succeeds with correct code-splitting. Awaiting human verification.
files_changed:
  - admin-ui/src/routes/_authenticated/services.tsx (replaced list content with Outlet layout)
  - admin-ui/src/routes/_authenticated/services.index.tsx (new file, list content moved here)
  - admin-ui/src/routeTree.gen.ts (auto-regenerated)
