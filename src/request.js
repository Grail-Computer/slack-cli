/**
 * Request encoding helpers for Slack Web API methods.
 */

const FORM_ENCODED_METHODS = new Set([
  "files.getUploadURLExternal",
  "files.completeUploadExternal",
]);

export function usesFormEncoding(method) {
  return FORM_ENCODED_METHODS.has(method);
}

export function formEncodeParams(params) {
  const body = new URLSearchParams();
  for (const [key, value] of Object.entries(params)) {
    if (value === undefined || value === null) continue;
    body.set(key, typeof value === "object" ? JSON.stringify(value) : String(value));
  }
  return body.toString();
}
