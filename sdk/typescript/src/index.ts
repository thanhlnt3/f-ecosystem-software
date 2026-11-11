/**
 * F-SDK cho TypeScript / JavaScript (spec app-sdk; design D7). License MIT OR Apache-2.0.
 *
 * Viết thuần trên chuẩn OAuth 2.0 / OpenID Connect và `fetch`, không phụ thuộc lõi Rust, nên chạy
 * được ở Node.js 18+ và trình duyệt. Chỉ dùng cú pháp TypeScript "xóa được" (Node 22.6+ chạy thẳng
 * tệp .ts; công cụ build thông thường cũng biên dịch được).
 *
 * ```ts
 * import { Session, configFromEnv } from "@f-eco/sdk";
 * const session = await Session.auto(configFromEnv());
 * const me = await session.me();
 * ```
 */

export const DEFAULT_REDIRECT_URI = "http://127.0.0.1:47280/callback";
export const DEFAULT_SCOPES = "openid offline_access profile";

export interface Config {
  clientId: string;
  /** Máy chủ OAuth 2.0 / OIDC (issuer). */
  issuer: string;
  /** API F-Eco. */
  server: string;
  redirectUri?: string;
  scopes?: string;
}

/** Lỗi API F-Eco theo định dạng thống nhất, hoặc lỗi của F-SDK. */
export class FsdkError extends Error {
  readonly error: string;
  readonly messageVi: string;
  readonly requestId: string;
  readonly status: number;

  constructor(error: string, message: string, messageVi = message, requestId = "", status = 0) {
    super(`${error}: ${message}`);
    this.name = "FsdkError";
    this.error = error;
    this.messageVi = messageVi;
    this.requestId = requestId;
    this.status = status;
  }
}

export interface Me {
  sub: string;
  name?: string | null;
  picture?: string | null;
  email?: string;
  email_verified?: boolean;
  locale?: string;
  zoneinfo?: string;
  phone_number?: string | null;
  [field: string]: unknown;
}

export interface Entitlement {
  kind: string;
  source?: string | null;
  granted_at?: string | null;
  expires_at?: string | null;
}

interface Discovery {
  authorization_endpoint: string;
  token_endpoint: string;
  revocation_endpoint?: string;
}

interface TokenResponse {
  access_token: string;
  refresh_token?: string;
  id_token?: string;
  expires_in?: number;
  scope?: string;
}

type Env = Record<string, string | undefined>;

function processEnv(): Env {
  const g = globalThis as { process?: { env?: Env } };
  return g.process?.env ?? {};
}

function slash(url: string): string {
  return url.endsWith("/") ? url : `${url}/`;
}

/** Cấu hình từ biến môi trường mà F-Launcher đặt khi mở app (Node.js). */
export function configFromEnv(env: Env = processEnv()): Config {
  const need = (k: string): string => {
    const v = env[k];
    if (!v) throw new FsdkError("sdk_error", `missing ${k}`, `thiếu biến môi trường ${k}`);
    return v;
  };
  return {
    clientId: need("FECO_CLIENT_ID"),
    issuer: need("FECO_ISSUER_URL"),
    server: need("FECO_SERVER_URL"),
    redirectUri: env.FSDK_REDIRECT_URI || DEFAULT_REDIRECT_URI,
    scopes: env.FSDK_SCOPES || DEFAULT_SCOPES,
  };
}

function base64url(bytes: Uint8Array): string {
  let s = "";
  for (const b of bytes) s += String.fromCharCode(b);
  return btoa(s).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
}

function randomString(bytes = 32): string {
  const buf = new Uint8Array(bytes);
  crypto.getRandomValues(buf);
  return base64url(buf);
}

/** PKCE (RFC 7636), dùng Web Crypto (có ở Node.js và trình duyệt). */
export async function pkce(): Promise<{ verifier: string; challenge: string }> {
  const verifier = randomString(32);
  const digest = await crypto.subtle.digest("SHA-256", new TextEncoder().encode(verifier));
  return { verifier, challenge: base64url(new Uint8Array(digest)) };
}

async function discover(issuer: string): Promise<Discovery> {
  const res = await fetch(new URL(".well-known/openid-configuration", slash(issuer)));
  if (!res.ok) throw new FsdkError("sdk_error", `discovery returned ${res.status}`, `khám phá OIDC trả ${res.status}`);
  return (await res.json()) as Discovery;
}

async function apiError(res: Response): Promise<FsdkError> {
  const text = await res.text();
  try {
    const b = JSON.parse(text) as Record<string, string>;
    return new FsdkError(b.error ?? "unexpected_response", b.message ?? text, b.message_vi ?? b.message ?? text, b.request_id ?? "", res.status);
  } catch {
    return new FsdkError("unexpected_response", text.slice(0, 200), text.slice(0, 200), "", res.status);
  }
}

async function tokenRequest(endpoint: string, form: Record<string, string>): Promise<TokenResponse> {
  const res = await fetch(endpoint, {
    method: "POST",
    headers: { "content-type": "application/x-www-form-urlencoded" },
    body: new URLSearchParams(form).toString(),
  });
  const body = (await res.json()) as Record<string, string>;
  if (!res.ok) {
    throw new FsdkError(body.error ?? "auth_error", body.error_description ?? "", body.error_description ?? "", "", res.status);
  }
  return body as unknown as TokenResponse;
}

/** Phiên đăng nhập: tự làm mới access token, gọi API F-Eco. */
export class Session {
  #config: Required<Config>;
  #discovery: Discovery;
  #access: string;
  #refresh?: string;
  #idToken?: string;
  #scope: string;
  #expiresAt: number;

  private constructor(config: Required<Config>, discovery: Discovery, t: TokenResponse) {
    this.#config = config;
    this.#discovery = discovery;
    this.#access = t.access_token;
    this.#refresh = t.refresh_token;
    this.#idToken = t.id_token;
    this.#scope = t.scope ?? "";
    this.#expiresAt = Date.now() + (t.expires_in ?? 3600) * 1000;
  }

  static #full(config: Config): Required<Config> {
    return {
      clientId: config.clientId,
      issuer: slash(config.issuer),
      server: slash(config.server),
      redirectUri: config.redirectUri ?? DEFAULT_REDIRECT_URI,
      scopes: config.scopes ?? DEFAULT_SCOPES,
    };
  }

  /** Phiên từ mã khởi chạy mà F-Launcher truyền (`FECO_LAUNCH_CODE`). */
  static async fromLaunchCode(config: Config, code: string): Promise<Session> {
    const c = Session.#full(config);
    const discovery = await discover(c.issuer);
    const { verifier, challenge } = await pkce();
    const res = await fetch(new URL("v1/launch-codes/exchange", c.server), {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ code, client_id: c.clientId, redirect_uri: c.redirectUri, code_challenge: challenge, scope: c.scopes }),
    });
    if (!res.ok) throw await apiError(res);
    const { authorization_code } = (await res.json()) as { authorization_code: string };
    const t = await tokenRequest(discovery.token_endpoint, {
      grant_type: "authorization_code",
      code: authorization_code,
      redirect_uri: c.redirectUri,
      client_id: c.clientId,
      code_verifier: verifier,
    });
    return new Session(c, discovery, t);
  }

  /** Phiên khi được F-Launcher mở (Node.js; đọc biến môi trường). */
  static async fromLauncher(env: Env = processEnv()): Promise<Session> {
    const code = env.FECO_LAUNCH_CODE;
    if (!code) throw new FsdkError("not_launched", "the app was not started by F-Launcher", "app không được F-Launcher khởi chạy");
    return Session.fromLaunchCode(configFromEnv(env), code);
  }

  /**
   * Đăng nhập qua trình duyệt bằng PKCE (Node.js): mở `open(url)` rồi chờ trình duyệt quay về
   * địa chỉ loopback `redirectUri`. Mặc định mở trình duyệt hệ thống.
   */
  static async loginWithBrowser(config: Config, open?: (url: string) => void | Promise<void>): Promise<Session> {
    const c = Session.#full(config);
    const redirect = new URL(c.redirectUri);
    if (redirect.protocol !== "http:" || !["127.0.0.1", "localhost"].includes(redirect.hostname)) {
      throw new FsdkError("sdk_error", "browser login needs a loopback redirect URI", "đăng nhập qua trình duyệt cần redirect_uri loopback");
    }
    const http = await import("node:http");
    const discovery = await discover(c.issuer);
    const { verifier, challenge } = await pkce();
    const state = randomString(16);
    const auth = new URL(discovery.authorization_endpoint);
    for (const [k, v] of Object.entries({
      client_id: c.clientId, response_type: "code", scope: c.scopes, redirect_uri: c.redirectUri,
      state, code_challenge: challenge, code_challenge_method: "S256",
    })) auth.searchParams.set(k, v);

    const params = await new Promise<URLSearchParams>((resolve, reject) => {
      const server = http.createServer((req, res) => {
        const url = new URL(req.url ?? "/", c.redirectUri);
        if (url.pathname !== redirect.pathname) {
          res.writeHead(404).end();
          return;
        }
        res.writeHead(200, { "content-type": "text/html; charset=utf-8" });
        res.end("<!doctype html><meta charset=utf-8><p>Đã đăng nhập. Hãy quay lại app.</p><p>Signed in. Return to the app.</p>");
        server.close();
        resolve(url.searchParams);
      });
      server.on("error", reject);
      server.listen(Number(redirect.port || 80), "127.0.0.1", async () => {
        try {
          await (open ?? openBrowser)(auth.toString());
        } catch (e) {
          server.close();
          reject(e);
        }
      });
    });
    if (params.get("error")) throw new FsdkError(params.get("error") ?? "auth_error", params.get("error_description") ?? "");
    if (params.get("state") !== state) throw new FsdkError("auth_error", "state mismatch", "state không khớp");
    const t = await tokenRequest(discovery.token_endpoint, {
      grant_type: "authorization_code",
      code: params.get("code") ?? "",
      redirect_uri: c.redirectUri,
      client_id: c.clientId,
      code_verifier: verifier,
    });
    return new Session(c, discovery, t);
  }

  /** Qua launcher thì dùng mã khởi chạy; chạy thẳng (hoặc chưa đồng ý scope) thì trình duyệt. */
  static async auto(config: Config, env: Env = processEnv()): Promise<Session> {
    const code = env.FECO_LAUNCH_CODE;
    if (code) {
      try {
        return await Session.fromLaunchCode(config, code);
      } catch (e) {
        if (!(e instanceof FsdkError) || e.error !== "consent_required") throw e;
      }
    }
    return Session.loginWithBrowser(config);
  }

  /** Tiếp tục phiên từ refresh token đã lưu. */
  static async resume(config: Config, refreshToken: string): Promise<Session> {
    const c = Session.#full(config);
    const discovery = await discover(c.issuer);
    const t = await tokenRequest(discovery.token_endpoint, { grant_type: "refresh_token", refresh_token: refreshToken, client_id: c.clientId });
    return new Session(c, discovery, t);
  }

  get refreshToken(): string | undefined {
    return this.#refresh;
  }

  get idToken(): string | undefined {
    return this.#idToken;
  }

  get grantedScopes(): string {
    return this.#scope;
  }

  /** Đánh dấu access token đã hết hạn (dùng trong test làm mới token). */
  expireNow(): void {
    this.#expiresAt = 0;
  }

  /** Access token còn hạn; tự làm mới khi còn dưới 60 giây. */
  async accessToken(): Promise<string> {
    if (this.#expiresAt - Date.now() > 60_000) return this.#access;
    await this.refresh();
    return this.#access;
  }

  async refresh(): Promise<void> {
    if (!this.#refresh) throw new FsdkError("sdk_error", "no refresh token (request offline_access)", "không có refresh token (cần scope offline_access)");
    const t = await tokenRequest(this.#discovery.token_endpoint, {
      grant_type: "refresh_token",
      refresh_token: this.#refresh,
      client_id: this.#config.clientId,
    });
    this.#access = t.access_token;
    this.#refresh = t.refresh_token ?? this.#refresh;
    this.#idToken = t.id_token ?? this.#idToken;
    this.#scope = t.scope ?? this.#scope;
    this.#expiresAt = Date.now() + (t.expires_in ?? 3600) * 1000;
  }

  async #get(path: string): Promise<unknown> {
    const res = await fetch(new URL(path, this.#config.server), {
      headers: { authorization: `Bearer ${await this.accessToken()}` },
    });
    if (!res.ok) throw await apiError(res);
    return res.json();
  }

  /** `GET /v1/me`: chỉ các trường thuộc scope đã được đồng ý. */
  async me(): Promise<Me> {
    return (await this.#get("v1/me")) as Me;
  }

  /** `GET /v1/entitlements`: quyền của người dùng với chính app này (cần scope `purchases`). */
  async entitlements(): Promise<Entitlement[]> {
    const body = (await this.#get("v1/entitlements")) as { entitlements: Entitlement[] };
    return body.entitlements;
  }

  /** Báo một lần app gặp sự cố (tùy chọn). */
  async reportCrash(version: string): Promise<void> {
    const res = await fetch(new URL("v1/apps/crashes", this.#config.server), {
      method: "POST",
      headers: { authorization: `Bearer ${await this.accessToken()}`, "content-type": "application/json" },
      body: JSON.stringify({ version }),
    });
    if (!res.ok) throw await apiError(res);
  }
}

async function openBrowser(url: string): Promise<void> {
  const { spawn } = await import("node:child_process");
  const platform = (globalThis as { process?: { platform?: string } }).process?.platform;
  const [cmd, args] = platform === "win32" ? ["cmd", ["/c", "start", "", url]] : platform === "darwin" ? ["open", [url]] : ["xdg-open", [url]];
  spawn(cmd, args, { stdio: "ignore", detached: true }).unref();
}

/** Kiểm chữ ký webhook `F-Eco-Signature: t=<giây>,v1=<hex>` (HMAC-SHA256, lệch tối đa 5 phút). */
export async function verifyWebhook(secret: string, header: string, body: Uint8Array | string, nowSeconds = Math.floor(Date.now() / 1000)): Promise<boolean> {
  const parts = new Map(header.split(",").map((p) => p.trim().split("=", 2) as [string, string]));
  const t = Number(parts.get("t"));
  const v1 = parts.get("v1");
  if (!Number.isInteger(t) || !v1 || Math.abs(nowSeconds - t) > 300 || !/^[0-9a-f]+$/.test(v1)) return false;
  const enc = new TextEncoder();
  const key = await crypto.subtle.importKey("raw", enc.encode(secret), { name: "HMAC", hash: "SHA-256" }, false, ["verify"]);
  const data = typeof body === "string" ? enc.encode(body) : body;
  const message = new Uint8Array([...enc.encode(`${t}.`), ...data]);
  const signature = new Uint8Array(v1.match(/../g)!.map((h) => parseInt(h, 16)));
  // Web Crypto so sánh chữ ký trong thời gian hằng định.
  return crypto.subtle.verify("HMAC", key, signature, message);
}
