import assert from "node:assert/strict";
import { mkdtemp, writeFile } from "node:fs/promises";
import http from "node:http";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";

import { uploadFile } from "../src/upload.js";

async function localUploadServer(onUpload) {
  const server = http.createServer((request, response) => {
    const chunks = [];
    request.on("data", (chunk) => chunks.push(chunk));
    request.on("end", () => {
      onUpload(request, Buffer.concat(chunks));
      response.writeHead(200, { "Content-Type": "text/plain" });
      response.end("OK");
    });
  });
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  return server;
}

test("uploads bytes and shares the file with an optional caption", async (t) => {
  const fixture = Buffer.from("safe local fixture\n", "utf8");
  const fixtureDir = await mkdtemp(join(tmpdir(), "slk-upload-test-"));
  const fixturePath = join(fixtureDir, "fixture.txt");
  await writeFile(fixturePath, fixture);

  let received;
  const server = await localUploadServer((request, body) => {
    received = { method: request.method, headers: request.headers, body };
  });
  t.after(() => server.close());
  const { port } = server.address();

  const calls = [];
  const slackApi = async (method, params) => {
    calls.push({ method, params });
    if (method === "files.getUploadURLExternal") {
      return {
        ok: true,
        upload_url: `http://127.0.0.1:${port}/upload`,
        file_id: "F123FIXTURE",
      };
    }
    if (method === "files.completeUploadExternal") {
      return { ok: true, files: [{ id: "F123FIXTURE", title: "fixture.txt" }] };
    }
    throw new Error(`Unexpected method: ${method}`);
  };

  const result = await uploadFile("@alex", fixturePath, "Fixture caption", {
    resolveChannel: async (ref) => {
      assert.equal(ref, "@alex");
      return "D123FIXTURE";
    },
    slackApi,
  });

  assert.equal(received.method, "POST");
  assert.equal(received.headers["content-type"], "application/octet-stream");
  assert.equal(received.headers["content-length"], String(fixture.length));
  assert.deepEqual(received.body, fixture);
  assert.deepEqual(calls, [
    {
      method: "files.getUploadURLExternal",
      params: { filename: "fixture.txt", length: fixture.length },
    },
    {
      method: "files.completeUploadExternal",
      params: {
        files: [{ id: "F123FIXTURE", title: "fixture.txt" }],
        channel_id: "D123FIXTURE",
        initial_comment: "Fixture caption",
      },
    },
  ]);
  assert.deepEqual(result, {
    channel: "D123FIXTURE",
    file: { id: "F123FIXTURE", title: "fixture.txt" },
  });
});

test("rejects a missing file before resolving a channel or calling Slack", async () => {
  let contactedSlack = false;
  await assert.rejects(
    uploadFile("general", "/definitely/not/a/file.png", "", {
      resolveChannel: async () => {
        contactedSlack = true;
      },
      slackApi: async () => {
        contactedSlack = true;
      },
    }),
    /File not found/
  );
  assert.equal(contactedSlack, false);
});

test("does not finalize when the byte transfer fails", async (t) => {
  const fixtureDir = await mkdtemp(join(tmpdir(), "slk-upload-test-"));
  const fixturePath = join(fixtureDir, "fixture.txt");
  await writeFile(fixturePath, "fixture");

  const server = http.createServer((_request, response) => {
    response.writeHead(500);
    response.end("failed");
  });
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  t.after(() => server.close());
  const { port } = server.address();

  const methods = [];
  await assert.rejects(
    uploadFile("general", fixturePath, "", {
      resolveChannel: async () => "C123FIXTURE",
      slackApi: async (method) => {
        methods.push(method);
        return {
          ok: true,
          upload_url: `http://127.0.0.1:${port}/upload`,
          file_id: "F123FIXTURE",
        };
      },
    }),
    /File transfer failed \(HTTP 500\)/
  );
  assert.deepEqual(methods, ["files.getUploadURLExternal"]);
});
