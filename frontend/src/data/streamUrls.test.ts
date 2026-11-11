import { describe, expect, test } from "vitest";
import { cleanUrl, isValidHttpUrl, isValidUrl, isValidWebSocketUrl, urlPort } from "./streamUrls";

describe("isValidUrl", () => {
  test.each([
    ["srt://my_public_ip:4000", null],
    ["srtla://foobar.org:4432", null],
    ["srt://foobar.org", "SRT(LA) port number missing"],
    ["rist://foobar.org", "RIST port number missing"],
    ["rist://foobar.org:2030", null],
    ["rtmp://my_public_ip:1935/live/foobar", null],
    ["rtmp://foobar.org", null],
    ["whip://foobar.org/live", null],
    ["mobcam://localhost:7790", null],
    ["mobcam://foobar.org:7790", "Mobcam host must be localhost"],
    ["mobcam://127.0.0.1", "Mobcam port number missing"],
    ["https://foobar.org", null],
    ["foobar.org:4000", "Host missing"],
    ["ftp://foobar.org", "Unsupported scheme ftp"],
    ["srt://foobar.org:70000", "Malformed URL"],
    ["", "Malformed URL"],
  ])("%s", (url, error) => {
    expect(isValidUrl(url)).toBe(error);
  });

  test("allowed schemes", () => {
    expect(isValidUrl("srt://foobar.org:4000", ["rtmp", "rtmps"])).toBe(
      "Only rtmp and rtmps allowed, not srt",
    );
    expect(isValidUrl("rtmp://foobar.org/app", ["rtmp", "rtmps"], false)).toBe(null);
  });
});

describe("other URLs", () => {
  test("WebSocket", () => {
    expect(isValidWebSocketUrl("")).toBe(null);
    expect(isValidWebSocketUrl("ws://foo.org:5443/ws")).toBe(null);
    expect(isValidWebSocketUrl("http://foo.org")).toBe("Unsupported scheme http");
  });

  test("HTTP", () => {
    expect(isValidHttpUrl("https://rtirl.com/api")).toBe(null);
    expect(isValidHttpUrl("ws://rtirl.com")).toBe("Unsupported scheme ws");
  });

  test("clean", () => {
    expect(cleanUrl("SRT://Foo.org: 4000")).toBe("srt://Foo.org:4000");
  });

  test("port", () => {
    expect(urlPort("mobcam://localhost:7790")).toBe(7790);
    expect(urlPort("mobcam://localhost")).toBe(null);
  });
});
