export interface ClosableRuntime {
  close(): Promise<void>;
}

export function installShutdownHandlers(
  runtime: ClosableRuntime,
  timeoutMs: number,
  exit: (code: number) => never = process.exit,
): () => void {
  let shuttingDown = false;

  const shutdown = async (): Promise<void> => {
    if (shuttingDown) return;
    shuttingDown = true;
    const timeout = setTimeout(() => exit(1), timeoutMs);
    timeout.unref();
    try {
      await runtime.close();
      clearTimeout(timeout);
      exit(0);
    } catch {
      clearTimeout(timeout);
      exit(1);
    }
  };

  const onSignal = (): void => void shutdown();
  process.once("SIGINT", onSignal);
  process.once("SIGTERM", onSignal);

  return () => {
    process.off("SIGINT", onSignal);
    process.off("SIGTERM", onSignal);
  };
}
