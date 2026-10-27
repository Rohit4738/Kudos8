// Tiny in-memory router. OWNER: Session A.
// Session D: call `navigate()` from pages; do not add a router dependency.

export type Route =
  | { name: "home" }
  | { name: "game"; id: string }
  | { name: "settings" };

interface RouterState {
  route: Route;
  history: Route[];
}

export const router = $state<RouterState>({
  route: { name: "home" },
  history: [],
});

export function navigate(route: Route): void {
  const current = router.route;
  if (current.name === route.name) {
    if (current.name !== "game") return;
    if (route.name === "game" && current.id === route.id) return;
  }
  router.history.push(current);
  router.route = route;
}

export function back(): void {
  const previous = router.history.pop();
  router.route = previous ?? { name: "home" };
}

export function isActive(name: Route["name"]): boolean {
  return router.route.name === name;
}
