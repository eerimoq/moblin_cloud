import assert from "node:assert/strict";
import { mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { spawnSync } from "node:child_process";
import { runInNewContext } from "node:vm";

const directory = mkdtempSync(join(tmpdir(), "moblin-config-"));
const output = join(directory, "config.js");
const script = readFileSync(new URL("../runtime-config.sh", import.meta.url), "utf8").replace(
  "/srv/config.js",
  output,
);
const flags = ["SELFHOSTED", "TIPPING_ENABLED"];
const env = Object.fromEntries(Object.entries(process.env).filter(([key]) => !flags.includes(key)));
const run = (values) =>
  spawnSync("sh", ["-s", "--", "true"], {
    input: script,
    env: { ...env, ...values },
    encoding: "utf8",
  });
const config = () => {
  const context = { window: {} };
  runInNewContext(readFileSync(output, "utf8"), context);
  return JSON.parse(JSON.stringify(context.window.config));
};
try {
  assert.equal(run({}).status, 0);
  assert.deepEqual(config(), {
    backendUrl: "/api",
    selfHosted: false,
    tippingEnabled: true,
  });
  assert.equal(run(Object.fromEntries(flags.map((key) => [key, "false"]))).status, 0);
  assert.deepEqual(config(), {
    backendUrl: "/api",
    selfHosted: false,
    tippingEnabled: false,
  });
  assert.equal(run({ SELFHOSTED: "true" }).status, 0);
  assert.equal(config().selfHosted, true);
  assert.equal(run(Object.fromEntries(flags.map((key) => [key, "false"]))).status, 0);
  for (const flag of flags) {
    const result = run({ [flag]: "false; echo unsafe" });
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /must be true or false/);
    assert.equal(config().tippingEnabled, false);
  }
  console.log("PASS: runtime defaults, disabled features and invalid environment values");
} finally {
  rmSync(directory, { recursive: true, force: true });
}
