import { createHash } from "node:crypto";
import type { JsonValue } from "./types.js";

/**
 * Return a SHA-256 hex digest of a JSON value.
 *
 * Uses {@link stableStringify} internally so two semantically equivalent
 * objects produce the same hash regardless of key insertion order.
 */
export function hashJson(value: JsonValue): string {
  return createHash("sha256").update(stableStringify(value)).digest("hex");
}

/**
 * Stringify JSON with object keys sorted recursively so semantically
 * equivalent inputs produce the same bytes before SHA-256 hashing or
 * persistence.
 */
export function stableStringify(value: JsonValue): string {
  if (value === null || typeof value !== "object") {
    return JSON.stringify(value);
  }

  if (Array.isArray(value)) {
    return `[${value.map((item) => stableStringify(item)).join(",")}]`;
  }

  const entries = Object.entries(value)
    .filter((entry): entry is [string, JsonValue] => entry[1] !== undefined)
    .sort(([left], [right]) => left.localeCompare(right));

  return `{${entries
    .map(([key, item]) => `${JSON.stringify(key)}:${stableStringify(item)}`)
    .join(",")}}`;
}

/**
 * Extract a human-readable error message from an unknown throw value.
 *
 * Returns `error.stack` (if available) for `Error` instances, otherwise falls
 * back to `String(error)`.
 */
export function errorMessage(error: unknown): string {
  if (error instanceof Error) {
    return error.stack ?? error.message;
  }

  return String(error);
}

/**
 * Build a readable error string from an HTTP `Response`.
 *
 * Preferentially extracts `body.error` when the response body is JSON;
 * otherwise falls back to plain text or a generic HTTP status message.
 */
export async function responseErrorMessage(
  response: Response,
): Promise<string> {
  const text = await response.text();
  if (text === "") {
    return `HTTP ${response.status}`;
  }

  try {
    const body = JSON.parse(text) as { error?: string };
    return body.error ?? text;
  } catch {
    return text;
  }
}
