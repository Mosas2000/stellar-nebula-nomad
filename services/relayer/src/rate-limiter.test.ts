import { InMemoryRateLimiter, RedisRateLimiter } from "./rate-limiter";

describe("InMemoryRateLimiter", () => {
  it("allows calls under the limit", async () => {
    const limiter = new InMemoryRateLimiter(3, 60_000);
    for (let i = 0; i < 3; i++) {
      const result = await limiter.checkAndRecord("addr:GABC");
      expect(result.allowed).toBe(true);
    }
  });

  it("rejects calls beyond the limit within the window", async () => {
    const limiter = new InMemoryRateLimiter(2, 60_000);
    await limiter.checkAndRecord("addr:GABC");
    await limiter.checkAndRecord("addr:GABC");
    const third = await limiter.checkAndRecord("addr:GABC");

    expect(third.allowed).toBe(false);
    expect(third.remaining).toBe(0);
    expect(third.retryAfterMs).toBeGreaterThan(0);
  });

  it("tracks independent keys separately", async () => {
    const limiter = new InMemoryRateLimiter(1, 60_000);
    const first = await limiter.checkAndRecord("addr:GABC");
    const second = await limiter.checkAndRecord("addr:GXYZ");

    expect(first.allowed).toBe(true);
    expect(second.allowed).toBe(true);
  });

  it("allows calls again once the window has elapsed", async () => {
    const limiter = new InMemoryRateLimiter(1, 50);
    const first = await limiter.checkAndRecord("addr:GABC");
    expect(first.allowed).toBe(true);

    const blocked = await limiter.checkAndRecord("addr:GABC");
    expect(blocked.allowed).toBe(false);

    await new Promise((resolve) => setTimeout(resolve, 60));

    const afterWindow = await limiter.checkAndRecord("addr:GABC");
    expect(afterWindow.allowed).toBe(true);
  });

  it("rejects non-positive constructor arguments", () => {
    expect(() => new InMemoryRateLimiter(0, 1000)).toThrow();
    expect(() => new InMemoryRateLimiter(5, 0)).toThrow();
    expect(() => new InMemoryRateLimiter(-1, 1000)).toThrow();
  });

  it("reports decreasing remaining count", async () => {
    const limiter = new InMemoryRateLimiter(5, 60_000);
    const first = await limiter.checkAndRecord("k");
    const second = await limiter.checkAndRecord("k");
    expect(first.remaining).toBe(4);
    expect(second.remaining).toBe(3);
  });
});

describe("RedisRateLimiter", () => {
  function makeMockClient() {
    const store = new Map<string, Array<{ score: number; value: string }>>();

    return {
      store,
      zRemRangeByScore: jest.fn(async (key: string, _min: number, max: number) => {
        const entries = store.get(key) ?? [];
        const kept = entries.filter((e) => e.score > max);
        store.set(key, kept);
        return entries.length - kept.length;
      }),
      zCard: jest.fn(async (key: string) => (store.get(key) ?? []).length),
      zRangeWithScores: jest.fn(async (key: string, start: number, stop: number) => {
        const entries = [...(store.get(key) ?? [])].sort((a, b) => a.score - b.score);
        return entries.slice(start, stop + 1);
      }),
      zAdd: jest.fn(async (key: string, member: { score: number; value: string }) => {
        const entries = store.get(key) ?? [];
        entries.push(member);
        store.set(key, entries);
        return 1;
      }),
      pExpire: jest.fn(async () => true),
    };
  }

  it("allows calls under the limit and records them in the sorted set", async () => {
    const client = makeMockClient();
    const limiter = new RedisRateLimiter(client as never, 2, 60_000);

    const first = await limiter.checkAndRecord("addr:GABC");
    const second = await limiter.checkAndRecord("addr:GABC");

    expect(first.allowed).toBe(true);
    expect(second.allowed).toBe(true);
    expect(client.zAdd).toHaveBeenCalledTimes(2);
  });

  it("rejects once the sorted-set count reaches the limit", async () => {
    const client = makeMockClient();
    const limiter = new RedisRateLimiter(client as never, 1, 60_000);

    await limiter.checkAndRecord("addr:GABC");
    const second = await limiter.checkAndRecord("addr:GABC");

    expect(second.allowed).toBe(false);
    expect(second.remaining).toBe(0);
  });

  it("expires old entries via zRemRangeByScore before counting", async () => {
    const client = makeMockClient();
    const limiter = new RedisRateLimiter(client as never, 1, 60_000);

    await limiter.checkAndRecord("addr:GABC");
    await limiter.checkAndRecord("addr:GABC");

    expect(client.zRemRangeByScore).toHaveBeenCalledTimes(2);
  });

  it("rejects non-positive constructor arguments", () => {
    const client = makeMockClient();
    expect(() => new RedisRateLimiter(client as never, 0, 1000)).toThrow();
    expect(() => new RedisRateLimiter(client as never, 5, 0)).toThrow();
  });
});
