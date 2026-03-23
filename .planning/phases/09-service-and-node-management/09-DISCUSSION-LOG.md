# Phase 9: Service and Node Management - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-03-23
**Phase:** 09-service-and-node-management
**Areas discussed:** Service list layout, Service detail page

---

## Service List Layout

### Display format

| Option | Description | Selected |
|--------|-------------|----------|
| Data table | shadcn/ui DataTable with sortable columns (name, description, nodes, queue, created) | |
| Card grid | One card per service with name, description, node/queue stats, health badge | ✓ |
| Simple list | Minimal list with service name and key stats | |

**User's choice:** Card grid
**Notes:** More visual approach preferred over tabular data

### Health indicator

| Option | Description | Selected |
|--------|-------------|----------|
| Color dot + label | Green/yellow/red dots with Healthy/Degraded/Down labels | ✓ |
| Node ratio only | Colored number ratio (3/5 nodes) | |
| You decide | Claude picks | |

**User's choice:** Color dot + label
**Notes:** Matches DASH-03 health badge pattern for consistency

### Register button placement

| Option | Description | Selected |
|--------|-------------|----------|
| Top-right of page | Primary button in page header next to title | ✓ |
| Floating action button | Fixed '+' in bottom-right corner | |
| You decide | Claude picks | |

**User's choice:** Top-right of page

### Empty state

| Option | Description | Selected |
|--------|-------------|----------|
| Guide to register | EmptyState with icon + heading + CTA button | ✓ |
| Just text | EmptyState with guidance text only | |
| You decide | Claude picks | |

**User's choice:** Guide to register with CTA button

---

## Service Detail Page

### Page organization

| Option | Description | Selected |
|--------|-------------|----------|
| Single page with sections | Config at top, node table below, scrollable | ✓ |
| Tabbed layout | Tabs for Overview and Nodes | |
| Side-by-side | Config left, nodes right | |

**User's choice:** Single page with sections

### Node health display

| Option | Description | Selected |
|--------|-------------|----------|
| Color dot + status text | Green/yellow/red/blue dots with status labels | ✓ |
| Badge/pill style | Colored pill badges | |
| You decide | Claude picks | |

**User's choice:** Color dot + status text

### Node detail level

| Option | Description | Selected |
|--------|-------------|----------|
| Table is sufficient | All NODE-02 info visible in table columns | ✓ |
| Expandable row | Click to expand for extra info | |
| Sheet/drawer | Side sheet with full details | |

**User's choice:** Table is sufficient — no separate node detail needed

### Navigation

| Option | Description | Selected |
|--------|-------------|----------|
| Click card → /services/$name | Navigate to detail page with breadcrumb back | ✓ |
| Click card → sheet/drawer | Open detail in overlay | |

**User's choice:** Click card → /services/$name with breadcrumb

---

## Claude's Discretion

- Service registration form layout and field grouping
- Deregister confirmation dialog wording
- Card grid responsive breakpoints
- Post-deregister UI behavior (optimistic removal vs poll)
- Loading skeleton designs
- Last seen time formatting
- Queue depth fetching strategy

## Deferred Ideas

- Service config editing (EDIT-01) — deferred from v1.1
- Node drain/disconnect UI actions — not in requirements
- Queue depth sparklines on cards — Phase 12 enhancement
