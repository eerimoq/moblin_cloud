import { afterEach, beforeEach, describe, expect, test, vi } from "vitest";
import { createStreams, type Streams } from "./store";
import { BackendError } from "../../data/backend";
import type { Control, DeviceSettings } from "../../data/account";

const kickId = "2B3C4D5E-6F7A-4B8C-9D0E-1F2A3B4C5D6E";
const twitchId = "1A2B3C4D-5E6F-4A7B-8C9D-0E1F2A3B4C5D";
const newId = "3C4D5E6F-7A8B-4C9D-0E1F-2A3B4C5D6E7F";

type Moblin = {
  streams: Record<string, Record<string, unknown>>;
  refuse: (control: Control) => boolean;
  unreachable: boolean;
  sent: Control[];
};

const settings = (moblin: Moblin, current = twitchId): DeviceSettings => ({
  streams: Object.values(moblin.streams).map((stream) => ({
    id: stream.id as string,
    name: stream.name as string,
  })),
  currentStreamId: current,
  scenes: [],
  bitratePresets: [],
  mics: [],
  srt: { connectionPrioritiesEnabled: false, connectionPriorities: [] },
});

const kindOf = (control: Control) => Object.keys(control)[0];

const setUp = (readOnly = false) => {
  const moblin: Moblin = {
    streams: {
      [twitchId]: { id: twitchId, name: "Twitch", url: "srt://a:1" },
      [kickId]: { id: kickId, name: "Kick", url: "rtmps://b/app/key" },
    },
    refuse: () => false,
    unreachable: false,
    sent: [],
  };
  const request = async <T>(control: Control): Promise<T> => {
    moblin.sent.push(control);
    if (moblin.unreachable) {
      throw new BackendError(504, "Moblin did not answer. Try again.");
    }
    if (moblin.refuse(control)) {
      throw new BackendError(409, "Moblin refused the change. Reload the streams and try again.");
    }
    const body = Object.values(control)[0] as Record<string, unknown>;
    switch (kindOf(control)) {
      case "getStream": {
        const stream = moblin.streams[body.id as string];
        if (!stream) {
          throw new BackendError(
            409,
            "Moblin refused the change. Reload the streams and try again.",
          );
        }
        return { stream: structuredClone(stream) } as T;
      }
      case "getEditorOptions":
        return { options: { showAllSettings: true } } as T;
      case "createStream":
      case "updateStream": {
        const stream = body.stream as Record<string, unknown>;
        moblin.streams[stream.id as string] = stream;
        return undefined as T;
      }
      case "deleteStream":
        delete moblin.streams[body.id as string];
        return undefined as T;
      default:
        return undefined as T;
    }
  };
  let current = twitchId;
  const control = async (sent: Control) => {
    moblin.sent.push(sent);
    if (kindOf(sent) === "setStream") {
      current = (Object.values(sent)[0] as { id: string }).id;
    }
  };
  const streams = createStreams(
    request,
    control,
    () => settings(moblin, current),
    () => readOnly,
  );
  return { moblin, streams };
};

const kinds = (moblin: Moblin) => moblin.sent.map(kindOf);

const changes = (moblin: Moblin) =>
  kinds(moblin).filter((kind) => !["getStream", "getEditorOptions"].includes(kind));

const listed = (streams: Streams) => streams.list().map((item) => [item.name, item.enabled]);

describe("streams store", () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  test("the list is the device's streams, with the one in use enabled", () => {
    const { streams } = setUp();
    expect(listed(streams)).toEqual([
      ["Twitch", true],
      ["Kick", false],
    ]);
  });

  test("a stream is fetched when opened and an unchanged one is never sent back", async () => {
    const { moblin, streams } = setUp();
    const stream = await streams.load(kickId);
    expect(stream?.url).toBe("rtmps://b/app/key");
    await streams.flush();
    expect(kinds(moblin)).toEqual(["getStream", "getEditorOptions"]);
  });

  test("an edit is sent as the whole stream", async () => {
    const { moblin, streams } = setUp();
    await streams.load(kickId);
    streams.edit(kickId, (draft) => {
      draft.fps = 60;
    });
    await streams.flush();
    expect(changes(moblin)).toEqual(["updateStream"]);
    expect(moblin.streams[kickId].fps).toBe(60);
    expect(listed(streams)).toEqual([
      ["Twitch", true],
      ["Kick", false],
    ]);
  });

  test("a deleted stream's name can be taken at once, since deletes are sent first", async () => {
    const { moblin, streams } = setUp();
    moblin.refuse = (control) =>
      kindOf(control) === "createStream" &&
      Object.values(moblin.streams).some((stream) => stream.name === "Kick");
    const kick = await streams.load(kickId);
    streams.remove(kickId);
    streams.add({ ...structuredClone(kick!), id: newId, enabled: false });
    expect(listed(streams)).toEqual([
      ["Twitch", true],
      ["Kick", false],
    ]);
    await streams.flush();
    expect(changes(moblin)).toEqual(["deleteStream", "createStream"]);
    expect(Object.keys(moblin.streams)).toEqual([twitchId, newId]);
    expect(streams.state.error).toBe(null);
  });

  test("a refused update takes Moblin's stream and does not block the others", async () => {
    const { moblin, streams } = setUp();
    await streams.load(kickId);
    await streams.load(twitchId);
    moblin.refuse = (control) =>
      kindOf(control) === "updateStream" &&
      (control as { updateStream: { stream: { id: string } } }).updateStream.stream.id === kickId;
    streams.edit(kickId, (draft) => {
      draft.fps = 60;
    });
    streams.edit(twitchId, (draft) => {
      draft.fps = 25;
    });
    await streams.flush();
    expect(streams.stream(kickId)?.fps).toBe(30);
    expect(moblin.streams[twitchId].fps).toBe(25);
    expect(streams.state.error).toBe(
      "Moblin refused the change. Reload the streams and try again.",
    );
    const sent = moblin.sent.length;
    await vi.advanceTimersByTimeAsync(10_000);
    expect(moblin.sent.length).toBe(sent);
  });

  test("a stream deleted on the phone is forgotten when its update is refused", async () => {
    const { moblin, streams } = setUp();
    await streams.load(kickId);
    delete moblin.streams[kickId];
    moblin.refuse = (control) => kindOf(control) === "updateStream";
    streams.edit(kickId, (draft) => {
      draft.fps = 60;
    });
    await streams.flush();
    expect(streams.stream(kickId)).toBe(undefined);
    expect(listed(streams)).toEqual([["Twitch", true]]);
  });

  test("a refused create is dropped", async () => {
    const { moblin, streams } = setUp();
    moblin.refuse = (control) => kindOf(control) === "createStream";
    const kick = await streams.load(kickId);
    streams.add({ ...structuredClone(kick!), id: newId, name: "Copy" });
    streams.select(newId);
    await streams.flush();
    expect(listed(streams)).toEqual([
      ["Twitch", true],
      ["Kick", false],
    ]);
    expect(kinds(moblin)).not.toContain("setStream");
  });

  test("a refused delete brings the stream back", async () => {
    const { moblin, streams } = setUp();
    moblin.refuse = (control) => kindOf(control) === "deleteStream";
    streams.remove(kickId);
    expect(listed(streams)).toEqual([["Twitch", true]]);
    await streams.flush();
    expect(listed(streams)).toEqual([
      ["Twitch", true],
      ["Kick", false],
    ]);
  });

  test("Moblin not answering retries the whole sync after 5 s", async () => {
    const { moblin, streams } = setUp();
    await streams.load(kickId);
    moblin.unreachable = true;
    streams.edit(kickId, (draft) => {
      draft.fps = 60;
    });
    await streams.flush();
    expect(streams.state.error).toBe("Moblin did not answer. Try again.");
    moblin.unreachable = false;
    await vi.advanceTimersByTimeAsync(5_000);
    expect(moblin.streams[kickId].fps).toBe(60);
    expect(streams.state.error).toBe(null);
  });

  test("selecting a known stream switches at once", async () => {
    const { moblin, streams } = setUp();
    streams.select(kickId);
    expect(listed(streams)).toEqual([
      ["Twitch", false],
      ["Kick", true],
    ]);
    await vi.runAllTimersAsync();
    expect(kinds(moblin)).toEqual(["setStream"]);
  });

  test("a new stream is selected only once it is created", async () => {
    const { moblin, streams } = setUp();
    const kick = await streams.load(kickId);
    streams.add({ ...structuredClone(kick!), id: newId, name: "Copy" });
    streams.select(newId);
    await streams.flush();
    await vi.runAllTimersAsync();
    expect(changes(moblin)).toEqual(["createStream", "setStream"]);
  });

  test("a read-only viewer can open streams but changes nothing", async () => {
    const { moblin, streams } = setUp(true);
    const kick = await streams.load(kickId);
    streams.edit(kickId, (draft) => {
      draft.fps = 60;
    });
    streams.add({ ...structuredClone(kick!), id: newId, name: "Copy" });
    streams.remove(twitchId);
    streams.select(kickId);
    await streams.flush();
    await vi.runAllTimersAsync();
    expect(streams.stream(kickId)?.fps).toBe(30);
    expect(changes(moblin)).toEqual([]);
    expect(listed(streams)).toEqual([
      ["Twitch", true],
      ["Kick", false],
    ]);
  });

  test("an edit made while the stream is being fetched is kept and sent", async () => {
    const { moblin, streams } = setUp();
    await streams.load(kickId);
    moblin.streams[kickId] = { ...moblin.streams[kickId], name: "Kick on the phone" };
    const loading = streams.load(kickId);
    streams.edit(kickId, (draft) => {
      draft.fps = 60;
    });
    await loading;
    await streams.flush();
    expect(streams.stream(kickId)?.fps).toBe(60);
    expect(moblin.streams[kickId].fps).toBe(60);
  });
});
