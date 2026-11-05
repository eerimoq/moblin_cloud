import { afterEach, describe, expect, it, vi } from "vitest";

afterEach(() => vi.unstubAllGlobals());

describe("policyUrl", () => {
  it("allows absolute web URLs and rejects executable or invalid links", async () => {
    vi.stubGlobal("window", { config: { backendUrl: "/api" } });
    const { policyUrl } = await import("./links");
    expect(policyUrl(" https://example.com/terms ")).toBe("https://example.com/terms");
    expect(policyUrl("http://localhost/privacy")).toBe("http://localhost/privacy");
    for (const value of [
      undefined,
      "",
      "/terms",
      "not a URL",
      "javascript:alert(1)",
      "data:text/html,test",
      "ftp://example.com",
    ]) {
      expect(policyUrl(value)).toBeUndefined();
    }
  });
});
