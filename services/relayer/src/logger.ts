import { RelayLogEntry } from "./types";

/**
 * Minimal structured JSON logger. Every relay attempt (accepted, rejected,
 * submitted, failed) is logged as one JSON line to stdout so it can be
 * shipped to any log aggregator for fraud-monitoring visibility, without
 * pulling in a logging framework for what is a handful of fields.
 */
export function logRelayEvent(entry: Omit<RelayLogEntry, "timestamp">): void {
  const full = { ...entry, timestamp: new Date().toISOString() };
  // eslint-disable-next-line no-console
  console.log(JSON.stringify(full));
}
