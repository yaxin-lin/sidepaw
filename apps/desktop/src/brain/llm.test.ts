import { describe, expect, it } from "vitest";
import { approxTokens, complete, openaiBody, openaiText, preview, resolve, sseDeltas, stream, type Deps } from "./llm";

describe("resolve", () => {
  it("fills provider defaults and rejects what cannot work", () => {
    expect(resolve({ provider: "deepseek", model: "", base_url: null })).toEqual({
      provider: "deepseek",
      model: "deepseek-chat",
      baseUrl: "https://api.deepseek.com/v1",
      local: false,
    });
    expect(() => resolve({ provider: "", model: "", base_url: null })).toThrow("no model provider");
    expect(() => resolve({ provider: "ollama", model: "", base_url: null })).toThrow("needs a model name");
    expect(() => resolve({ provider: "custom", model: "x", base_url: "" })).toThrow("base URL");
  });

  it("treats loopback URLs as local and strips trailing slashes", () => {
    const r = resolve({ provider: "custom", model: "qwen", base_url: "http://localhost:8080/v1/" });
    expect(r.baseUrl).toBe("http://localhost:8080/v1");
    expect(r.local).toBe(true);
    expect(resolve({ provider: "ollama", model: "qwen3", base_url: null }).local).toBe(true);
    expect(resolve({ provider: "custom", model: "m", base_url: "https://example.com/v1" }).local).toBe(false);
  });
});

describe("openai-compatible body", () => {
  const req = { purpose: "lines" as const, system: "sys", user: "hi", maxTokens: 100 };
  it("uses max_completion_tokens only for OpenAI", () => {
    const openai = openaiBody(resolve({ provider: "openai", model: "", base_url: null }), req);
    expect(openai.max_completion_tokens).toBe(100);
    expect(openai.max_tokens).toBeUndefined();
    const ds = openaiBody(resolve({ provider: "deepseek", model: "", base_url: null }), req);
    expect(ds.max_tokens).toBe(100);
    expect(ds.messages).toEqual([
      { role: "system", content: "sys" },
      { role: "user", content: "hi" },
    ]);
  });

  it("reads the first choice and rejects anything else", () => {
    expect(openaiText({ choices: [{ message: { content: "yo" } }] })).toBe("yo");
    expect(() => openaiText({ choices: [] })).toThrow("no text");
  });
});

describe("preview and tokens", () => {
  it("counts CJK per character and the rest by fours", () => {
    expect(approxTokens("abcd")).toBe(1);
    expect(approxTokens("你好")).toBe(2);
    expect(approxTokens("你好abcd")).toBe(3);
  });

  it("shows exactly what goes out, history included", () => {
    const p = preview({ provider: "deepseek", model: "", base_url: null }, { purpose: "chat", system: "S", user: "U", maxTokens: 1 });
    expect(p.text).toBe("[system]\nS\n\n[user]\nU");
    const withHistory = preview(
      { provider: "deepseek", model: "", base_url: null },
      { purpose: "chat", system: "S", history: [{ role: "user", content: "a" }, { role: "assistant", content: "b" }], user: "U", maxTokens: 1 },
    );
    expect(withHistory.text).toBe("[system]\nS\n\n[user]\na\n\n[assistant]\nb\n\n[user]\nU");
    expect(p.bytes).toBe(p.text.length);
    expect(p.local).toBe(false);
  });
});

describe("sseDeltas", () => {
  it("collects content deltas and skips noise", () => {
    const lines = [
      "event: ping",
      'data: {"choices":[{"delta":{"role":"assistant"}}]}',
      'data: {"choices":[{"delta":{"content":"Hel"}}]}',
      "data: not json",
      'data: {"choices":[{"delta":{"content":"lo"}}]}',
      "data: [DONE]",
    ].join("\n");
    expect(sseDeltas(lines)).toEqual(["Hel", "lo"]);
  });
});

describe("stream", () => {
  it("delivers deltas across chunk boundaries and audits once", async () => {
    const audits: unknown[] = [];
    const chunks = ['data: {"choices":[{"delta":{"content":"a"}}]}\ndata: {"choi', 'ces":[{"delta":{"content":"b"}}]}\n\ndata: [DONE]\n'];
    const body = new ReadableStream<Uint8Array>({
      start(c) {
        for (const ch of chunks) c.enqueue(new TextEncoder().encode(ch));
        c.close();
      },
    });
    const deps: Deps = { fetch: async () => new Response(body, { status: 200 }), audit: async (...a) => void audits.push(a) };
    const got: string[] = [];
    const text = await stream({ provider: "deepseek", model: "", base_url: null }, { purpose: "chat", system: "S", user: "U", maxTokens: 5 }, "k", (d) => got.push(d), deps);
    expect(got).toEqual(["a", "b"]);
    expect(text).toBe("ab");
    expect(audits).toHaveLength(1);
  });
});

describe("complete", () => {
  const cfg = { provider: "deepseek", model: "", base_url: null };
  const req = { purpose: "persona" as const, system: "S", user: "U", maxTokens: 50 };

  function deps(res: Response, calls: unknown[] = [], audits: unknown[] = []): Deps {
    return {
      fetch: async (input, init) => {
        calls.push([String(input), init]);
        return res;
      },
      audit: async (...a) => {
        audits.push(a);
      },
    };
  }

  it("audits before sending and returns the text", async () => {
    const calls: unknown[] = [];
    const audits: unknown[] = [];
    const res = new Response(JSON.stringify({ choices: [{ message: { content: "meow" } }] }), { status: 200 });
    const text = await complete(cfg, req, "sk-test", deps(res, calls, audits));
    expect(text).toBe("meow");
    expect(audits).toHaveLength(1);
    const [provider, purpose, bytes, sha] = audits[0] as [string, string, number, string];
    expect([provider, purpose, bytes]).toEqual(["deepseek", "persona", "[system]\nS\n\n[user]\nU".length]);
    expect(sha).toMatch(/^[0-9a-f]{64}$/);
    const [url, init] = calls[0] as [string, RequestInit];
    expect(url).toBe("https://api.deepseek.com/v1/chat/completions");
    expect((init.headers as Record<string, string>).authorization).toBe("Bearer sk-test");
  });

  it("refuses to send without a key where one is needed", async () => {
    const audits: unknown[] = [];
    await expect(complete(cfg, req, null, deps(new Response("{}"), [], audits))).rejects.toThrow("API key");
    expect(audits).toHaveLength(0);
  });

  it("surfaces the provider's error message", async () => {
    const res = new Response(JSON.stringify({ error: { message: "insufficient balance" } }), { status: 402 });
    await expect(complete(cfg, req, "k", deps(res))).rejects.toThrow("HTTP 402 insufficient balance");
  });
});
