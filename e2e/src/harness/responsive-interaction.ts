export interface ResponsiveInteractionOptions<T> {
  acquire: () => Promise<T | null>;
  interact: (current: T) => Promise<boolean>;
  waitUntil: (attempt: () => Promise<boolean>) => Promise<void>;
}

function isResponsiveTransition(error: unknown): boolean {
  const message = error instanceof Error ? error.message : String(error);
  return /stale element reference|element not interactable|element click intercepted|no such element/i.test(
    message,
  );
}

/** Reacquire responsive controls after the rendered branch changes. */
export async function retryResponsiveInteraction<T>({
  acquire,
  interact,
  waitUntil,
}: ResponsiveInteractionOptions<T>): Promise<void> {
  await waitUntil(async () => {
    try {
      const current = await acquire();
      if (current === null) return false;
      return await interact(current);
    } catch (error) {
      if (!isResponsiveTransition(error)) throw error;
      return false;
    }
  });
}
