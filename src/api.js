/**
 * Slack API wrapper — handles auth, retries on invalid_auth, and pagination.
 */

import { getCredentials, refresh } from "./auth.js";
import { formEncodeParams, usesFormEncoding } from "./request.js";

const BASE = "https://slack.com/api";

/**
 * Make an authenticated Slack API call.
 * Auto-refreshes credentials on invalid_auth (once).
 */
export async function slackApi(method, params = {}, retried = false) {
  const { token, cookie } = getCredentials();

  const url = new URL(`${BASE}/${method}`);

  // GET for read methods, POST for write methods
  const writeMethods = [
    "chat.postMessage",
    "chat.update",
    "chat.delete",
    "reactions.add",
    "reactions.remove",
    "files.upload",
    "files.getUploadURLExternal",
    "files.completeUploadExternal",
    "drafts.create",
    "drafts.delete",
    "drafts.update",
    "conversations.open",
    "client.counts",
    "users.prefs.get",
    "saved.list",
  ];

  const isWrite = writeMethods.some((m) => method.startsWith(m));
  let res;

  if (isWrite) {
    const formEncoded = usesFormEncoding(method);
    res = await fetch(url, {
      method: "POST",
      headers: {
        Authorization: `Bearer ${token}`,
        Cookie: `d=${cookie}`,
        "Content-Type": formEncoded
          ? "application/x-www-form-urlencoded"
          : "application/json; charset=utf-8",
      },
      body: formEncoded ? formEncodeParams(params) : JSON.stringify(params),
    });
  } else {
    for (const [k, v] of Object.entries(params)) {
      if (v !== undefined && v !== null) url.searchParams.set(k, v);
    }
    res = await fetch(url, {
      headers: {
        Authorization: `Bearer ${token}`,
        Cookie: `d=${cookie}`,
      },
    });
  }

  const data = await res.json();

  if (!data.ok && data.error === "invalid_auth" && !retried) {
    refresh();
    return slackApi(method, params, true);
  }

  return data;
}

/**
 * Paginate through a Slack API method using cursor-based pagination.
 */
export async function slackPaginate(method, params = {}, key = "channels") {
  const results = [];
  let cursor;

  do {
    const data = await slackApi(method, { ...params, cursor, limit: params.limit || 200 });
    if (!data.ok) return data; // return error as-is

    if (data[key]) results.push(...data[key]);
    cursor = data.response_metadata?.next_cursor;
  } while (cursor);

  return { ok: true, [key]: results };
}
