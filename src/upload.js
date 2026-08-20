/**
 * Slack's external file-upload flow.
 */

import { createReadStream } from "fs";
import { stat } from "fs/promises";
import { basename } from "path";

async function uploadBytes(uploadUrl, filePath, size, fetchImpl = fetch) {
  const response = await fetchImpl(uploadUrl, {
    method: "POST",
    headers: {
      "Content-Type": "application/octet-stream",
      "Content-Length": String(size),
    },
    body: createReadStream(filePath),
    duplex: "half",
  });

  if (!response.ok) {
    throw new Error(`File transfer failed (HTTP ${response.status})`);
  }
}

function slackError(data) {
  const details = data.response_metadata?.messages
    ?.map((message) => message.replace(/^\[ERROR\]\s*/, ""))
    .join("; ");
  return details ? `${data.error} (${details})` : data.error;
}

/**
 * Validate, upload, and share one file through Slack's external upload APIs.
 * Dependencies are passed in so the complete flow can be tested without Slack.
 */
export async function uploadFile(
  channelRef,
  filePath,
  caption,
  { resolveChannel, slackApi, fetchImpl = fetch }
) {
  let fileInfo;
  try {
    fileInfo = await stat(filePath);
  } catch (err) {
    if (err.code === "ENOENT") throw new Error(`File not found: ${filePath}`);
    throw err;
  }

  if (!fileInfo.isFile()) throw new Error(`Not a file: ${filePath}`);
  if (fileInfo.size === 0) throw new Error(`Cannot upload an empty file: ${filePath}`);

  const filename = basename(filePath);
  const channel = await resolveChannel(channelRef);
  const ticket = await slackApi("files.getUploadURLExternal", {
    filename,
    length: fileInfo.size,
  });

  if (!ticket.ok) {
    throw new Error(`Failed to start upload: ${slackError(ticket)}`);
  }

  await uploadBytes(ticket.upload_url, filePath, fileInfo.size, fetchImpl);

  const params = {
    files: [{ id: ticket.file_id, title: filename }],
    channel_id: channel,
  };
  if (caption) params.initial_comment = caption;

  const completed = await slackApi("files.completeUploadExternal", params);
  if (!completed.ok) {
    throw new Error(`Failed to complete upload: ${slackError(completed)}`);
  }

  return { channel, file: completed.files?.[0] };
}
