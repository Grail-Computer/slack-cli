import assert from "node:assert/strict";
import test from "node:test";

import { formEncodeParams, usesFormEncoding } from "../src/request.js";

test("external upload API methods use form encoding", () => {
  assert.equal(usesFormEncoding("files.getUploadURLExternal"), true);
  assert.equal(usesFormEncoding("files.completeUploadExternal"), true);
  assert.equal(usesFormEncoding("chat.postMessage"), false);
});

test("form encoding preserves upload fields and JSON-encodes structured values", () => {
  const encoded = formEncodeParams({
    filename: "clip finder.png",
    length: 42,
    files: [{ id: "F123", title: "clip finder.png" }],
    channel_id: "D123",
    initial_comment: "Here it is",
  });
  const params = new URLSearchParams(encoded);

  assert.equal(params.get("filename"), "clip finder.png");
  assert.equal(params.get("length"), "42");
  assert.equal(
    params.get("files"),
    JSON.stringify([{ id: "F123", title: "clip finder.png" }])
  );
  assert.equal(params.get("channel_id"), "D123");
  assert.equal(params.get("initial_comment"), "Here it is");
});
