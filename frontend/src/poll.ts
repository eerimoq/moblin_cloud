export function poll(refresh: () => void, intervalMs: number) {
  let timer: ReturnType<typeof setInterval> | undefined;
  const stop = () => {
    clearInterval(timer);
    timer = undefined;
  };
  const start = () => {
    stop();
    timer = setInterval(refresh, intervalMs);
  };
  const onVisibility = () => {
    if (document.hidden) {
      stop();
    } else {
      refresh();
      start();
    }
  };
  document.addEventListener("visibilitychange", onVisibility);
  if (!document.hidden) {
    start();
  }
  return () => {
    stop();
    document.removeEventListener("visibilitychange", onVisibility);
  };
}
