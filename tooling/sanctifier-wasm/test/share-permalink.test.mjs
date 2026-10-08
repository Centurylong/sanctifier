import assert from "node:assert/strict";
import test from "node:test";
import {
  MAX_SHARE_SOURCE_BYTES,
  decodeShareHash,
  encodeShareHash,
  makeShareUrl,
} from "../demo/permalink.js";

test("browser source permalink: UTF-8 round trip, bounded payload and invalid input", () => {
  const source = "fn check() { /* Soroban 🛡️ café */ }\n";
  const fragment = encodeShareHash(source);
  assert.match(fragment, /^#code=v1\.[A-Za-z0-9_-]+$/);
  assert.equal(decodeShareHash(fragment), source);
  const url = makeShareUrl(source, "https://example.org/demo/?origin=docs");
  assert.equal(new URL(url).searchParams.get("origin"), "docs");
  assert.equal(decodeShareHash(new URL(url).hash), source);

  assert.equal(decodeShareHash("#tab=details"), null);
  assert.throws(() => decodeShareHash("#code=v2.abc"), /unsupported/);
  assert.throws(() => decodeShareHash("#code=v1.!invalid"), /invalid/);
  assert.throws(() => encodeShareHash(""), /Enter contract/);
  assert.throws(() => encodeShareHash("x".repeat(MAX_SHARE_SOURCE_BYTES + 1)), /4 KiB/);
  assert.throws(() => decodeShareHash("#code=v1." + "A".repeat(6000)), /oversized/);
});
