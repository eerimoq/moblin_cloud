type EventHandler = (name: string, data: unknown) => void;

type StreamHandlers = {
  onEvent: EventHandler;
  onError?: (status: number | null) => void;
  onOpen?: () => void;
};

export function subscribe(url: string, token: string | null, handlers: StreamHandlers): () => void {
  let controller: AbortController | null = null;
  let timer: ReturnType<typeof setTimeout> | undefined;
  let stopped = false;
  let refused = false;
  let attempts = 0;

  const schedule = () => {
    if (stopped || refused || document.hidden) {
      return;
    }
    clearTimeout(timer);
    timer = setTimeout(open, Math.min(1000 * 2 ** attempts, 10_000));
    attempts += 1;
  };

  const open = async () => {
    if (stopped || document.hidden || controller) {
      return;
    }
    const current = new AbortController();
    controller = current;
    try {
      const response = await fetch(url, {
        headers: {
          ...(token === null ? {} : { authorization: `Bearer ${token}` }),
          accept: "text/event-stream",
        },
        signal: current.signal,
      });
      if (!response.ok || !response.body) {
        refused = response.status >= 400 && response.status < 500;
        handlers.onError?.(response.status);
        throw new Error(String(response.status));
      }
      attempts = 0;
      handlers.onOpen?.();
      const reader = response.body.pipeThrough(new TextDecoderStream()).getReader();
      let buffer = "";
      for (;;) {
        const { value, done } = await reader.read();
        if (done) {
          break;
        }
        buffer += value;
        for (;;) {
          const end = buffer.indexOf("\n\n");
          if (end < 0) {
            break;
          }
          dispatch(buffer.slice(0, end));
          buffer = buffer.slice(end + 2);
        }
      }
      handlers.onError?.(null);
    } catch (error) {
      if (!current.signal.aborted && !(error instanceof Error && /^\d+$/.test(error.message))) {
        handlers.onError?.(null);
      }
    } finally {
      if (controller === current) {
        controller = null;
      }
      schedule();
    }
  };

  const dispatch = (block: string) => {
    let name = "message";
    const data: string[] = [];
    for (const line of block.split("\n")) {
      if (line.startsWith("event:")) {
        name = line.slice(6).trim();
      } else if (line.startsWith("data:")) {
        data.push(line.slice(5).trimStart());
      }
    }
    if (data.length > 0) {
      handlers.onEvent(name, JSON.parse(data.join("\n")));
    }
  };

  const close = () => {
    clearTimeout(timer);
    controller?.abort();
    controller = null;
  };

  const onVisibility = () => {
    if (document.hidden) {
      close();
    } else {
      attempts = 0;
      refused = false;
      void open();
    }
  };

  document.addEventListener("visibilitychange", onVisibility);
  void open();
  return () => {
    stopped = true;
    document.removeEventListener("visibilitychange", onVisibility);
    close();
  };
}

export function subscribeChanges(
  url: string,
  token: string | null,
  refreshes: Record<string, () => void>,
  onError?: (status: number | null) => void,
): () => void {
  let opened = false;
  return subscribe(url, token, {
    onOpen: () => {
      if (opened) {
        new Set(Object.values(refreshes)).forEach((refresh) => refresh());
      }
      opened = true;
    },
    onEvent: (name) => refreshes[name]?.(),
    onError,
  });
}
