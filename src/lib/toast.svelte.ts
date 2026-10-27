// Tiny toast queue. OWNER: Session D.
// Rendered by <Toaster /> at the top level of pages that need feedback.

export type ToastKind = "error" | "success" | "info";

export interface Toast {
  id: number;
  kind: ToastKind;
  title: string;
  message?: string;
}

let nextId = 1;
const timers = new Map<number, ReturnType<typeof setTimeout>>();

export const toasts = $state<Toast[]>([]);

export function pushToast(
  kind: ToastKind,
  title: string,
  message?: string,
  duration = 5200,
): number {
  const id = nextId++;
  toasts.push({ id, kind, title, message });
  // Keep the stack short; the newest feedback is the relevant one.
  if (toasts.length > 4) {
    const oldest = toasts[0];
    if (oldest) dismissToast(oldest.id);
  }
  timers.set(
    id,
    setTimeout(() => dismissToast(id), duration),
  );
  return id;
}

export function dismissToast(id: number): void {
  const timer = timers.get(id);
  if (timer) {
    clearTimeout(timer);
    timers.delete(id);
  }
  const index = toasts.findIndex((toast) => toast.id === id);
  if (index >= 0) toasts.splice(index, 1);
}
