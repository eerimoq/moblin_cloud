import {
  createContext,
  createSignal,
  For,
  Index,
  type JSX,
  Match,
  onCleanup,
  Show,
  Switch,
  useContext,
} from "solid-js";
import type { RecentKind } from "../data/account";
import { t } from "../i18n";

export const panelKinds = ["phone", "activity", "graphs", "status", "message", "web"] as const;

export type PanelKind = (typeof panelKinds)[number];

export type Leaf = {
  panel: PanelKind;
  device?: string;
  kinds?: RecentKind[];
  url?: string;
  twitchChat?: boolean;
  hidePreview?: boolean;
};

export type Split = { split: "row" | "column"; sizes: number[]; children: Pane[] };

export type Pane = Leaf | Split;

type Path = number[];

export type LeafActions = {
  change: (leaf: Leaf) => void;
  split: (direction: Split["split"]) => void;
  remove: (() => void) | null;
  drag: {
    draggable: true;
    onDragStart: (event: DragEvent) => void;
    onDragEnd: () => void;
  };
};

type Zone = "left" | "right" | "top" | "bottom" | "center";

const isSplit = (pane: Pane): pane is Split => "split" in pane;

const normalized = (sizes: number[]) => {
  const total = sizes.reduce((sum, size) => sum + size, 0);
  return sizes.map((size) => size / total);
};

export function sanitize(value: unknown, depth = 0): Pane | null {
  if (typeof value !== "object" || value === null || depth > 8) {
    return null;
  }
  const pane = value as Record<string, unknown>;
  if (pane.split === "row" || pane.split === "column") {
    const children = (Array.isArray(pane.children) ? pane.children : [])
      .map((child) => sanitize(child, depth + 1))
      .filter((child): child is Pane => child !== null)
      .slice(0, 8);
    if (children.length === 0) {
      return null;
    }
    if (children.length === 1) {
      return children[0]!;
    }
    const sizes = Array.isArray(pane.sizes) ? pane.sizes : [];
    return {
      split: pane.split,
      sizes: normalized(
        children.map((_, index) => {
          const size = sizes[index];
          return typeof size === "number" && size > 0.05 ? size : 1 / children.length;
        }),
      ),
      children,
    };
  }
  if (!panelKinds.includes(pane.panel as PanelKind)) {
    return null;
  }
  const leaf: Leaf = { panel: pane.panel as PanelKind };
  if (typeof pane.device === "string") {
    leaf.device = pane.device;
  }
  if (Array.isArray(pane.kinds)) {
    leaf.kinds = pane.kinds.filter((kind): kind is RecentKind =>
      ["tip", "spin", "effect"].includes(kind as string),
    );
  }
  if (typeof pane.url === "string" && pane.url.startsWith("https://")) {
    leaf.url = pane.url;
  }
  if (pane.twitchChat === true) {
    leaf.twitchChat = true;
  }
  if (pane.hidePreview === true) {
    leaf.hidePreview = true;
  }
  return leaf;
}

const leavesOf = (pane: Pane, path: Path = []): { leaf: Leaf; path: Path }[] =>
  isSplit(pane)
    ? pane.children.flatMap((child, index) => leavesOf(child, [...path, index]))
    : [{ leaf: pane, path }];

const updateAt = (pane: Pane, path: Path, update: (pane: Pane) => Pane): Pane => {
  if (path.length === 0) {
    return update(pane);
  }
  if (!isSplit(pane)) {
    return pane;
  }
  const [index, ...rest] = path;
  return {
    ...pane,
    children: pane.children.map((child, at) =>
      at === index ? updateAt(child, rest, update) : child,
    ),
  };
};

const splitAt = (
  root: Pane,
  path: Path,
  direction: Split["split"],
  added: Leaf,
  before = false,
): Pane => {
  const parentPath = path.slice(0, -1);
  const index = path[path.length - 1];
  return updateAt(root, parentPath, (parent) => {
    if (index !== undefined && isSplit(parent) && parent.split === direction) {
      const half = parent.sizes[index]! / 2;
      const sizes = [...parent.sizes];
      sizes.splice(index, 1, half, half);
      const children = [...parent.children];
      children.splice(before ? index : index + 1, 0, added);
      return { ...parent, sizes, children };
    }
    return updateAt(parent, index === undefined ? [] : [index], (leaf) => ({
      split: direction,
      sizes: [0.5, 0.5],
      children: before ? [added, leaf] : [leaf, added],
    }));
  });
};

const removeAt = (root: Pane, path: Path): Pane => {
  const index = path[path.length - 1];
  if (index === undefined) {
    return root;
  }
  return updateAt(root, path.slice(0, -1), (parent) => {
    if (!isSplit(parent)) {
      return parent;
    }
    const children = parent.children.filter((_, at) => at !== index);
    if (children.length === 1) {
      return children[0]!;
    }
    const sizes = [...parent.sizes];
    const [removed] = sizes.splice(index, 1);
    sizes[Math.max(0, index - 1)]! += removed!;
    return { ...parent, children, sizes };
  });
};

const nodeAt = (pane: Pane, path: Path): Pane | undefined =>
  path.length === 0
    ? pane
    : isSplit(pane)
      ? nodeAt(pane.children[path[0]!]!, path.slice(1))
      : undefined;

const pathOf = (pane: Pane, wanted: Pane, path: Path = []): Path | null => {
  if (pane === wanted) {
    return path;
  }
  if (!isSplit(pane)) {
    return null;
  }
  for (const [index, child] of pane.children.entries()) {
    const found = pathOf(child, wanted, [...path, index]);
    if (found) {
      return found;
    }
  }
  return null;
};

const moveTo = (root: Pane, from: Path, to: Path, zone: Zone): Pane => {
  const source = nodeAt(root, from);
  const target = nodeAt(root, to);
  if (!source || !target || source === target || isSplit(source) || isSplit(target)) {
    return root;
  }
  if (zone === "center") {
    return updateAt(
      updateAt(root, from, () => target),
      to,
      () => source,
    );
  }
  const removed = removeAt(root, from);
  const path = pathOf(removed, target);
  if (!path) {
    return root;
  }
  return splitAt(
    removed,
    path,
    zone === "left" || zone === "right" ? "row" : "column",
    source,
    zone === "left" || zone === "top",
  );
};

const moveToEdge = (root: Pane, from: Path, edge: Exclude<Zone, "center">): Pane => {
  const source = nodeAt(root, from);
  if (!source || isSplit(source) || from.length === 0) {
    return root;
  }
  const rest = removeAt(root, from);
  const direction = edge === "left" || edge === "right" ? "row" : "column";
  const first = edge === "left" || edge === "top";
  const share = 0.3;
  if (isSplit(rest) && rest.split === direction) {
    const sizes = rest.sizes.map((size) => size * (1 - share));
    return {
      ...rest,
      sizes: first ? [share, ...sizes] : [...sizes, share],
      children: first ? [source, ...rest.children] : [...rest.children, source],
    };
  }
  return {
    split: direction,
    sizes: first ? [share, 1 - share] : [1 - share, share],
    children: first ? [source, rest] : [rest, source],
  };
};

const edgeStrips: Record<Exclude<Zone, "center">, { strip: string; preview: string }> = {
  left: { strip: "inset-y-0 left-0 w-7", preview: "inset-y-0 left-0 w-[30%]" },
  right: { strip: "inset-y-0 right-0 w-7", preview: "inset-y-0 right-0 w-[30%]" },
  top: { strip: "inset-x-0 top-0 h-7", preview: "inset-x-0 top-0 h-[30%]" },
  bottom: { strip: "inset-x-0 bottom-0 h-7", preview: "inset-x-0 bottom-0 h-[30%]" },
};

function WindowEdges() {
  const dragging = useContext(DragContext)!;
  const [edge, setEdge] = createSignal<Exclude<Zone, "center"> | null>(null);
  return (
    <Show when={dragging.from()}>
      <For each={Object.keys(edgeStrips) as Exclude<Zone, "center">[]}>
        {(name) => (
          <div
            class={`absolute z-30 flex items-center justify-center ${edgeStrips[name].strip}`}
            onDragOver={(event) => {
              if (!dragging.movesToEdge(name)) {
                setEdge(null);
                return;
              }
              event.preventDefault();
              if (event.dataTransfer) {
                event.dataTransfer.dropEffect = "move";
              }
              setEdge(name);
            }}
            onDragLeave={() => setEdge(null)}
            onDrop={(event) => {
              event.preventDefault();
              setEdge(null);
              dragging.dropOnEdge(name);
            }}
          >
            <span
              class={`rounded-full bg-leaf/60 ${name === "left" || name === "right" ? "h-16 w-1" : "h-1 w-16"}`}
            />
          </div>
        )}
      </For>
      <Show when={edge()}>
        {(shown) => (
          <div
            class={`pointer-events-none absolute z-30 rounded-xl border-2 border-leaf bg-leaf/20 ${edgeStrips[shown()].preview}`}
          />
        )}
      </Show>
    </Show>
  );
}

const zoneAt = (event: DragEvent, element: HTMLElement): Zone => {
  const rect = element.getBoundingClientRect();
  const x = (event.clientX - rect.left) / rect.width;
  const y = (event.clientY - rect.top) / rect.height;
  if (x > 0.3 && x < 0.7 && y > 0.3 && y < 0.7) {
    return "center";
  }
  const edges: [Zone, number][] = [
    ["left", x],
    ["right", 1 - x],
    ["top", y],
    ["bottom", 1 - y],
  ];
  return edges.reduce((nearest, edge) => (edge[1] < nearest[1] ? edge : nearest))[0];
};

const zoneClasses: Record<Zone, string> = {
  left: "inset-y-0 left-0 w-1/2",
  right: "inset-y-0 right-0 w-1/2",
  top: "inset-x-0 top-0 h-1/2",
  bottom: "inset-x-0 bottom-0 h-1/2",
  center: "inset-0",
};

const arrangement = (pane: Pane, ids: Map<Leaf, number>): string => {
  if (!isSplit(pane)) {
    if (!ids.has(pane)) {
      ids.set(pane, ids.size);
    }
    return `${ids.get(pane)}`;
  }
  const parts = pane.children.map((child) =>
    isSplit(child) && child.split === pane.split
      ? arrangement(child, ids).slice(pane.split.length + 1, -1)
      : arrangement(child, ids),
  );
  return `${pane.split}(${parts.join(",")})`;
};

const sameArrangement = (a: Pane, b: Pane) => {
  const ids = new Map<Leaf, number>();
  return arrangement(a, ids) === arrangement(b, ids);
};

type Dragging = {
  from: () => Path | null;
  moves: (to: Path, zone: Zone) => boolean;
  movesToEdge: (edge: Exclude<Zone, "center">) => boolean;
  start: (path: Path) => void;
  end: () => void;
  drop: (to: Path, zone: Zone) => void;
  dropOnEdge: (edge: Exclude<Zone, "center">) => void;
};

const DragContext = createContext<Dragging>();

const samePath = (a: Path | null, b: Path) =>
  a !== null && a.length === b.length && a.every((index, at) => index === b[at]);

function LeafSlot(props: { path: Path; class?: string; children: JSX.Element }) {
  const dragging = useContext(DragContext)!;
  const [zone, setZone] = createSignal<Zone | null>(null);
  const source = () => samePath(dragging.from(), props.path);
  return (
    <div class={`relative flex min-h-0 min-w-0 p-[3px] ${props.class ?? "size-full"}`}>
      {props.children}
      <Show when={dragging.from()}>
        <div
          class={`absolute inset-[3px] z-20 rounded-xl ${source() ? "bg-page/50" : ""}`}
          onDragOver={(event) => {
            const at = zoneAt(event, event.currentTarget);
            if (source() || !dragging.moves(props.path, at)) {
              setZone(null);
              return;
            }
            event.preventDefault();
            if (event.dataTransfer) {
              event.dataTransfer.dropEffect = "move";
            }
            setZone(at);
          }}
          onDragLeave={() => setZone(null)}
          onDrop={(event) => {
            event.preventDefault();
            const dropped = zone();
            setZone(null);
            if (dropped && !source()) {
              dragging.drop(props.path, dropped);
            }
          }}
        >
          <Show when={zone()}>
            {(shown) => (
              <div
                class={`pointer-events-none absolute rounded-xl border-2 border-leaf bg-leaf/20 ${zoneClasses[shown()]}`}
              />
            )}
          </Show>
        </div>
      </Show>
    </div>
  );
}

const minimum = 0.08;

function SplitView(props: {
  split: Split;
  path: Path;
  root: () => Pane;
  onChange: (layout: Pane) => void;
  leaf: (leaf: () => Leaf, actions: LeafActions) => JSX.Element;
  added: () => Leaf;
}) {
  const [dragged, setDragged] = createSignal<number[] | null>(null);
  const sizes = () => dragged() ?? props.split.sizes;
  const row = () => props.split.split === "row";
  let container: HTMLDivElement | undefined;

  const commit = (next: number[]) =>
    props.onChange(
      updateAt(props.root(), props.path, (pane) => ({ ...(pane as Split), sizes: next })),
    );

  const resized = (start: number[], index: number, delta: number) => {
    const pair = start[index - 1]! + start[index]!;
    const before = Math.min(pair - minimum, Math.max(minimum, start[index - 1]! + delta));
    const next = [...start];
    next[index - 1] = before;
    next[index] = pair - before;
    return next;
  };

  const startDrag = (index: number, event: PointerEvent) => {
    if (!container) {
      return;
    }
    event.preventDefault();
    const handle = event.currentTarget as HTMLElement;
    handle.setPointerCapture(event.pointerId);
    const rect = container.getBoundingClientRect();
    const length = row() ? rect.width : rect.height;
    const origin = row() ? event.clientX : event.clientY;
    const start = sizes();
    const move = (moved: PointerEvent) => {
      const position = row() ? moved.clientX : moved.clientY;
      setDragged(resized(start, index, (position - origin) / length));
    };
    const end = () => {
      handle.removeEventListener("pointermove", move);
      handle.removeEventListener("pointerup", end);
      handle.removeEventListener("pointercancel", end);
      const next = dragged();
      setDragged(null);
      if (next) {
        commit(next);
      }
    };
    handle.addEventListener("pointermove", move);
    handle.addEventListener("pointerup", end);
    handle.addEventListener("pointercancel", end);
  };

  const onKey = (index: number, event: KeyboardEvent) => {
    const step = { ArrowLeft: -0.02, ArrowUp: -0.02, ArrowRight: 0.02, ArrowDown: 0.02 }[event.key];
    if (step === undefined) {
      return;
    }
    event.preventDefault();
    commit(resized(sizes(), index, step));
  };

  const boundary = (index: number) =>
    sizes()
      .slice(0, index)
      .reduce((sum, size) => sum + size, 0) * 100;

  return (
    <div
      ref={(element) => (container = element)}
      class={`relative flex size-full min-h-0 min-w-0 ${row() ? "flex-row" : "flex-col"}`}
    >
      <Index each={props.split.children}>
        {(child, index) => (
          <div class="flex min-h-0 min-w-0" style={{ flex: `${sizes()[index] ?? 0} 1 0px` }}>
            <Node
              pane={child()}
              path={[...props.path, index]}
              root={props.root}
              onChange={props.onChange}
              leaf={props.leaf}
              added={props.added}
            />
          </div>
        )}
      </Index>
      <Index each={props.split.children.slice(1)}>
        {(_, at) => {
          const index = at + 1;
          return (
            <div
              role="separator"
              tabindex="0"
              aria-orientation={row() ? "vertical" : "horizontal"}
              aria-label={t("Resize")}
              aria-valuenow={Math.round(sizes()[index - 1]! * 100)}
              onPointerDown={(event) => startDrag(index, event)}
              onKeyDown={(event) => onKey(index, event)}
              class={`group absolute z-10 flex touch-none items-center justify-center outline-none ${
                row() ? "inset-y-0 w-1.5 cursor-col-resize" : "inset-x-0 h-1.5 cursor-row-resize"
              }`}
              style={
                row()
                  ? { left: `calc(${boundary(index)}% - 3px)` }
                  : { top: `calc(${boundary(index)}% - 3px)` }
              }
            >
              <span
                class={`rounded-full bg-transparent transition-colors group-hover:bg-leaf/60 group-focus-visible:bg-leaf ${
                  row() ? "h-10 w-0.5" : "h-0.5 w-10"
                }`}
              />
            </div>
          );
        }}
      </Index>
    </div>
  );
}

function Node(props: {
  pane: Pane;
  path: Path;
  root: () => Pane;
  onChange: (layout: Pane) => void;
  leaf: (leaf: () => Leaf, actions: LeafActions) => JSX.Element;
  added: () => Leaf;
}) {
  const dragging = useContext(DragContext)!;
  return (
    <Switch>
      <Match when={isSplit(props.pane) && props.pane}>
        {(split) => (
          <SplitView
            split={split()}
            path={props.path}
            root={props.root}
            onChange={props.onChange}
            leaf={props.leaf}
            added={props.added}
          />
        )}
      </Match>
      <Match when={!isSplit(props.pane) && props.pane}>
        {(leaf) => (
          <LeafSlot path={props.path}>
            {props.leaf(
              () => leaf() as Leaf,
              actionsOf(props, () => props.path, dragging),
            )}
          </LeafSlot>
        )}
      </Match>
    </Switch>
  );
}

const actionsOf = (
  props: { root: () => Pane; onChange: (layout: Pane) => void; added: () => Leaf },
  path: () => Path,
  dragging: Dragging,
): LeafActions => ({
  change: (leaf) => props.onChange(updateAt(props.root(), path(), () => leaf)),
  split: (direction) => props.onChange(splitAt(props.root(), path(), direction, props.added())),
  get remove() {
    return path().length > 0 ? () => props.onChange(removeAt(props.root(), path())) : null;
  },
  drag: {
    draggable: true,
    onDragStart: (event) => {
      event.dataTransfer?.setData("text/plain", t("Pane"));
      if (event.dataTransfer) {
        event.dataTransfer.effectAllowed = "move";
      }
      const from = path();
      setTimeout(() => dragging.start(from));
    },
    onDragEnd: () => dragging.end(),
  },
});

export default function Tiles(props: {
  layout: Pane;
  onChange: (layout: Pane) => void;
  leaf: (leaf: () => Leaf, actions: LeafActions) => JSX.Element;
}) {
  const query = matchMedia("(min-width: 768px)");
  const [wide, setWide] = createSignal(query.matches);
  const onQuery = () => setWide(query.matches);
  query.addEventListener("change", onQuery);
  onCleanup(() => query.removeEventListener("change", onQuery));

  const added = (): Leaf => {
    const shown = new Set(leavesOf(props.layout).map(({ leaf }) => leaf.panel));
    return { panel: panelKinds.find((kind) => !shown.has(kind)) ?? "web" };
  };
  const shared = { root: () => props.layout, onChange: props.onChange, added };
  const [from, setFrom] = createSignal<Path | null>(null);
  const changes = (moved: (from: Path) => Pane) => {
    const dragged = from();
    return dragged !== null && !sameArrangement(props.layout, moved(dragged));
  };
  const dragging: Dragging = {
    from,
    moves: (to, zone) => changes((dragged) => moveTo(props.layout, dragged, to, zone)),
    movesToEdge: (edge) => changes((dragged) => moveToEdge(props.layout, dragged, edge)),
    start: setFrom,
    end: () => setFrom(null),
    drop: (to, zone) => {
      const moved = from();
      setFrom(null);
      if (moved) {
        props.onChange(moveTo(props.layout, moved, to, zone));
      }
    },
    dropOnEdge: (edge) => {
      const moved = from();
      setFrom(null);
      if (moved) {
        props.onChange(moveToEdge(props.layout, moved, edge));
      }
    },
  };

  return (
    <DragContext.Provider value={dragging}>
      <Show
        when={wide()}
        fallback={
          <div class="flex h-full flex-col overflow-y-auto p-[3px]">
            <For each={leavesOf(props.layout)}>
              {(entry) => (
                <LeafSlot path={entry.path} class="h-[30rem] w-full shrink-0">
                  {props.leaf(
                    () => entry.leaf,
                    actionsOf(shared, () => entry.path, dragging),
                  )}
                </LeafSlot>
              )}
            </For>
          </div>
        }
      >
        <div class="relative flex size-full p-[3px]">
          <WindowEdges />
          <Node
            pane={props.layout}
            path={[]}
            root={shared.root}
            onChange={props.onChange}
            leaf={props.leaf}
            added={added}
          />
        </div>
      </Show>
    </DragContext.Provider>
  );
}
