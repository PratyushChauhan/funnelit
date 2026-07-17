import assert from "node:assert/strict";
import { describe, it } from "node:test";
import { assetName, platformKey, releaseTag } from "./platform.js";
import { parseSha256Sum } from "./download.js";

describe("platformKey", () => {
  it("maps supported platforms", () => {
    assert.equal(platformKey("linux", "x64"), "linux-x64");
    assert.equal(platformKey("darwin", "arm64"), "darwin-arm64");
    assert.equal(platformKey("darwin", "x64"), "darwin-x64");
    assert.equal(platformKey("win32", "x64"), null);
  });
});

describe("asset naming", () => {
  it("builds release asset names", () => {
    assert.equal(assetName("0.1.0", "linux-x64"), "sumeru-v0.1.0-linux-x64");
    assert.equal(releaseTag("0.1.0"), "v0.1.0");
  });
});

describe("parseSha256Sum", () => {
  it("reads gnu sha256sum lines", () => {
    const body = [
      "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa  sumeru-v0.1.0-linux-x64",
      "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb  other",
    ].join("\n");
    assert.equal(
      parseSha256Sum(body, "sumeru-v0.1.0-linux-x64"),
      "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    );
  });
});
