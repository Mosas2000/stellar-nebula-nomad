import type { RedisClientType } from "redis";
import { RateLimiter, RateLimitResult } from "./types";

/**
 * In-process sliding-window rate limiter. Keeps a rolling list of call
 * timestamps per key and trims entries older than `windowMs` on every
 * check — the same fixed-window-trim approach `bot_detection.rs` uses
 * on-chain for its action-timestamp windows, mirrored here for the
 * service-layer rate limit.
 *
 * Suitable for a single relayer instance. For horizontal scaling, use
 * `RedisRateLimiter` instead so all instances share one counter.
 */
export class InMemoryRateLimiter implements RateLimiter {
  private readonly hits: Map<string, number[]> = new Map();

  constructor(
    private readonly maxCalls: number,
    private readonly windowMs: number,
  ) {
    if (maxCalls <= 0) {
      throw new Error("maxCalls must be a positive integer");
    }
    if (windowMs <= 0) {
      throw new Error("windowMs must be a positive integer");
    }
  }

  async checkAndRecord(key: string): Promise<RateLimitResult> {
    const now = Date.now();
    const cutoff = now - this.windowMs;

    const existing = this.hits.get(key) ?? [];
    const recent = existing.filter((ts) => ts > cutoff);

    if (recent.length >= this.maxCalls) {
      const oldest = recent[0];
      this.hits.set(key, recent);
      return {
        allowed: false,
        remaining: 0,
        retryAfterMs: oldest + this.windowMs - now,
      };
    }

    recent.push(now);
    this.hits.set(key, recent);

    return {
      allowed: true,
      remaining: this.maxCalls - recent.length,
    };
  }

  /** Test/ops helper: drop all tracked state. */
  reset(): void {
    this.hits.clear();
  }
}

/**
 * Redis-backed sliding-window rate limiter using a sorted set per key
 * (score = call timestamp). Shares state across every relayer instance —
 * use this in any multi-instance deployment. Matches the `redis` client
 * already used as a dependency convention in `services/webhook`.
 */
export class RedisRateLimiter implements RateLimiter {
  constructor(
    private readonly client: RedisClientType,
    private readonly maxCalls: number,
    private readonly windowMs: number,
    private readonly keyPrefix: string = "relayer:ratelimit:",
  ) {
    if (maxCalls <= 0) {
      throw new Error("maxCalls must be a positive integer");
    }
    if (windowMs <= 0) {
      throw new Error("windowMs must be a positive integer");
    }
  }

  async checkAndRecord(key: string): Promise<RateLimitResult> {
    const redisKey = `${this.keyPrefix}${key}`;
    const now = Date.now();
    const cutoff = now - this.windowMs;

    // Drop expired entries, then count what's left in the window.
    await this.client.zRemRangeByScore(redisKey, 0, cutoff);
    const count = await this.client.zCard(redisKey);

    if (count >= this.maxCalls) {
      const oldestEntries = await this.client.zRangeWithScores(redisKey, 0, 0);
      const oldestScore = oldestEntries.length > 0 ? oldestEntries[0].score : now;
      return {
        allowed: false,
        remaining: 0,
        retryAfterMs: Math.max(0, oldestScore + this.windowMs - now),
      };
    }

    // Use the timestamp as both member (unique-ish via microjitter) and score.
    const member = `${now}:${Math.random().toString(36).slice(2)}`;
    await this.client.zAdd(redisKey, { score: now, value: member });
    await this.client.pExpire(redisKey, this.windowMs);

    return {
      allowed: true,
      remaining: this.maxCalls - (count + 1),
    };
  }
}
