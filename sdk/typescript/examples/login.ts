// App mẫu TypeScript dùng F-SDK: qua launcher thì đăng nhập bằng mã khởi chạy, chạy thẳng thì mở
// trình duyệt (PKCE); rồi in /v1/me (và quyền sử dụng nếu xin scope `purchases`) dạng JSON.
//
//   node sdk/typescript/examples/login.ts
import { FsdkError, Session, configFromEnv } from "../src/index.ts";

try {
  const config = configFromEnv();
  const session = await Session.auto(config);
  const result: Record<string, unknown> = { me: await session.me() };
  if ((config.scopes ?? "").split(" ").includes("purchases")) {
    result.entitlements = await session.entitlements();
  }
  console.log(JSON.stringify(result));
} catch (e) {
  if (e instanceof FsdkError) {
    console.error(`lỗi: ${e.error}: ${e.messageVi} (request_id ${e.requestId})`);
    process.exit(1);
  }
  throw e;
}
