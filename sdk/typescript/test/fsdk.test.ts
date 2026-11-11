// Test của F-SDK TypeScript với máy chủ OIDC giả (node:http). Luồng thật với Hydra được kiểm ở
// E2E `sdk_end_to_end` của feco-server.
//
//   node --test sdk/typescript/test/
import assert from "node:assert/strict";
import { createHmac } from "node:crypto";
import { createServer, type IncomingMessage, type ServerResponse } from "node:http";
import { test } from "node:test";
import { FsdkError, Session, configFromEnv, pkce, verifyWebhook } from "../src/index.ts";

async function body(req: IncomingMessage): Promise<string> {
  let s = "";
  for await (const chunk of req) s += chunk;
  return s;
}

/** Máy chủ giả: khám phá OIDC, token (mã + refresh, xoay refresh token), đổi mã khởi chạy, /v1/me. */
async function fakeServer(): Promise<{ url: string; close: () => void; state: { refreshes: number } }> {
  const state = { refreshes: 0 };
  let valid = "refresh-0";
  const server = createServer(async (req: IncomingMessage, res: ServerResponse) => {
    const url = new URL(req.url ?? "/", "http://x");
    const json = (status: number, v: unknown) => {
      res.writeHead(status, { "content-type": "application/json" });
      res.end(JSON.stringify(v));
    };
    const base = `http://127.0.0.1:${(server.address() as { port: number }).port}/`;
    if (url.pathname === "/.well-known/openid-configuration") {
      return json(200, { authorization_endpoint: `${base}oauth2/auth`, token_endpoint: `${base}oauth2/token` });
    }
    if (url.pathname === "/v1/launch-codes/exchange") {
      const b = JSON.parse(await body(req));
      if (b.code === "da-dung") return json(400, { error: "invalid_grant", message: "used", message_vi: "đã dùng", request_id: "r-1" });
      if (b.code === "chua-dong-y") return json(403, { error: "consent_required", message: "x", message_vi: "y", request_id: "r-2" });
      assert.equal(b.code_challenge.length, 43);
      return json(200, { authorization_code: "code-1" });
    }
    if (url.pathname === "/oauth2/token") {
      const form = new URLSearchParams(await body(req));
      if (form.get("grant_type") === "authorization_code") {
        assert.equal(form.get("code"), "code-1");
        assert.ok((form.get("code_verifier") ?? "").length >= 43);
      } else if (form.get("refresh_token") !== valid) {
        return json(400, { error: "invalid_grant", error_description: "revoked" });
      } else {
        state.refreshes++;
      }
      valid = `refresh-${state.refreshes + 1}`;
      return json(200, { access_token: `access-${state.refreshes}`, refresh_token: valid, expires_in: 3600, scope: "openid profile" });
    }
    if (url.pathname === "/v1/me") {
      if (req.headers.authorization === "Bearer het-han") return json(401, { error: "invalid_token", message: "expired", message_vi: "hết hạn", request_id: "r-3" });
      return json(200, { sub: "pairwise", name: "Lan", picture: null });
    }
    json(404, { error: "not_found" });
  });
  await new Promise<void>((r) => server.listen(0, "127.0.0.1", r));
  const port = (server.address() as { port: number }).port;
  return { url: `http://127.0.0.1:${port}/`, close: () => server.close(), state };
}

test("pkce", async () => {
  const p = await pkce();
  assert.equal(p.verifier.length, 43);
  assert.match(p.challenge, /^[A-Za-z0-9_-]{43}$/);
});

test("launch code, auto refresh and structured errors", async () => {
  const s = await fakeServer();
  try {
    const config = { clientId: "app", issuer: s.url, server: s.url };
    const session = await Session.fromLaunchCode(config, "ma-hop-le");
    assert.equal((await session.me()).name, "Lan");
    assert.equal(await session.accessToken(), "access-0");
    session.expireNow();
    assert.equal(await session.accessToken(), "access-1", "tự làm mới");
    assert.equal(s.state.refreshes, 1);
    await assert.rejects(Session.fromLaunchCode(config, "da-dung"), (e: unknown) =>
      e instanceof FsdkError && e.error === "invalid_grant" && e.requestId === "r-1" && e.status === 400);
    await assert.rejects(Session.fromLauncher({}), (e: unknown) => e instanceof FsdkError && e.error === "not_launched");
    assert.throws(() => configFromEnv({}), FsdkError);
  } finally {
    s.close();
  }
});

test("browser login with PKCE on a loopback address", async () => {
  const s = await fakeServer();
  try {
    const redirectUri = "http://127.0.0.1:47391/callback";
    const session = await Session.loginWithBrowser({ clientId: "app", issuer: s.url, server: s.url, redirectUri }, async (url) => {
      const auth = new URL(url);
      assert.equal(auth.searchParams.get("code_challenge_method"), "S256");
      // "Trình duyệt" quay về địa chỉ loopback với code và đúng state.
      setTimeout(() => fetch(`${redirectUri}?code=code-1&state=${auth.searchParams.get("state")}`), 10);
    });
    assert.equal((await session.me()).sub, "pairwise");
  } finally {
    s.close();
  }
});

test("webhook signatures", async () => {
  const body = '{"event":"user.revoked"}';
  const v1 = createHmac("sha256", "bi-mat").update(`1000.${body}`).digest("hex");
  assert.equal(await verifyWebhook("bi-mat", `t=1000,v1=${v1}`, body, 1100), true);
  assert.equal(await verifyWebhook("khac", `t=1000,v1=${v1}`, body, 1100), false);
  assert.equal(await verifyWebhook("bi-mat", `t=1000,v1=${v1}`, body, 2000), false);
  assert.equal(await verifyWebhook("bi-mat", "t=1000", body, 1000), false);
});
