import { describe, expect, test } from "vitest";
import {
  cloneStream,
  defaultStream,
  encodeStream,
  normalizeStream,
  streamProtocol,
} from "./moblinStreamSettings";

const streamId = "2B3C4D5E-6F7A-4B8C-9D0E-1F2A3B4C5D6E";

const roundTrip = (value: unknown) => JSON.parse(JSON.stringify(value)) as Record<string, unknown>;

describe("normalizeStream", () => {
  test("fills a bare stream with the defaults", () => {
    const stream = normalizeStream({ id: streamId, name: "Kick", enabled: true });
    expect(stream).toEqual({ ...defaultStream(streamId), name: "Kick", enabled: true });
  });

  test("normalizing an encoded stream again changes nothing", () => {
    const stream = normalizeStream({
      id: streamId,
      multiStreaming: { destinations: [{ name: "YouTube", url: "rtmp://a/b", enabled: true }] },
      whip: { headers: [{ name: "Authorization" }] },
    });
    expect(normalizeStream(roundTrip(encodeStream(stream)))).toEqual(stream);
  });

  test("enum values are Moblin's, not merged with the default case", () => {
    const stream = normalizeStream({
      id: streamId,
      srt: { adaptiveBitrate: { algorithm: { fastIrl: {} } } },
      whip: { httpTransport: { remoteControl: {} } },
    });
    expect([stream.srt.adaptiveBitrate.algorithm, stream.whip.httpTransport]).toEqual([
      { fastIrl: {} },
      { remoteControl: {} },
    ]);
  });

  test("keeps fields it does not know", () => {
    const stream = normalizeStream({ id: streamId, newerField: 3, srt: { newerSrtField: true } });
    expect(encodeStream(stream).newerField).toBe(3);
    expect((encodeStream(stream).srt as Record<string, unknown>).newerSrtField).toBe(true);
  });

  test("Moblin's priorities are kept", () => {
    const priorities = [{ id: "A", name: "Cellular", priority: 3, enabled: false }];
    const stream = normalizeStream({
      id: streamId,
      srt: { connectionPriorities: { enabled: true, priorities } },
    });
    expect(stream.srt.connectionPriorities.priorities).toEqual(priorities);
  });
});

describe("encodeStream", () => {
  test("leaves out the destination ids Moblin does not have", () => {
    const stream = normalizeStream({
      id: streamId,
      multiStreaming: { destinations: [{ name: "YouTube" }] },
    });
    expect(encodeStream(stream).multiStreaming).toEqual({
      destinations: [
        { name: "YouTube", url: "rtmp://my_public_ip:1935/live/foobar", enabled: false },
      ],
    });
  });
});

describe("cloneStream", () => {
  test("gets new ids and is neither enabled nor logged in", () => {
    const stream = { ...defaultStream(streamId), enabled: true, twitchLoggedIn: true };
    const copy = cloneStream(stream);
    expect(copy.id).not.toBe(streamId);
    expect([copy.enabled, copy.twitchLoggedIn]).toEqual([false, false]);
    expect(copy.srt.connectionPriorities.priorities[0].id).not.toBe(
      stream.srt.connectionPriorities.priorities[0].id,
    );
  });
});

describe("streamProtocol", () => {
  test.each([
    ["srtla://a:1", "SRT"],
    ["rtmps://a/b/c", "RTMP"],
    ["https://a", "RTMP"],
    ["whips://a", "WHIP"],
    ["rist://a:1", "RIST"],
    ["mobcam://localhost:7790", "Mobcam"],
  ])("%s", (url, expected) => {
    expect(streamProtocol({ ...defaultStream(streamId), url })).toBe(expected);
  });
});
