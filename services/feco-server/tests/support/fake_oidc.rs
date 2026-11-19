//! Nhà cung cấp OpenID Connect giả cho E2E (thay đổi oidc-login-and-token-reuse, design T4).
//!
//! Mỗi nhà cung cấp (`google`, `github`) có issuer `http://127.0.0.1:4499/<tên>` như trong
//! `infra/kratos/kratos.e2e-oidc.yml`: discovery, authorize (tự đồng ý với người dùng test
//! đặt bằng [`FakeOidc::next_user`]), token (ID token ký ES256 bằng khóa sinh lúc chạy), JWKS,
//! userinfo. Chỉ dùng trong test.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use aws_lc_rs::{
    rand::SystemRandom,
    signature::{EcdsaKeyPair, KeyPair, ECDSA_P256_SHA256_FIXED_SIGNING},
};
use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Redirect, Response},
    routing::{get, post},
    Form, Json, Router,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use serde_json::{json, Value};

pub const ADDR: &str = "127.0.0.1:4499";

#[derive(Clone)]
pub struct FakeOidc {
    inner: Arc<Inner>,
}

/// Mã hoặc access token đã cấp: (nhà cung cấp, claim, nonce).
type Grant = (String, Value, Option<String>);

struct Inner {
    key: EcdsaKeyPair,
    rng: SystemRandom,
    /// Người dùng sẽ "đăng nhập" ở lần authorize tới, theo nhà cung cấp.
    next: Mutex<HashMap<String, Value>>,
    /// Mã ủy quyền đã cấp: mã → (nhà cung cấp, claim, nonce).
    codes: Mutex<HashMap<String, Grant>>,
}

impl FakeOidc {
    pub async fn serve() -> FakeOidc {
        let rng = SystemRandom::new();
        let pkcs8 = EcdsaKeyPair::generate_pkcs8(&ECDSA_P256_SHA256_FIXED_SIGNING, &rng).unwrap();
        let key =
            EcdsaKeyPair::from_pkcs8(&ECDSA_P256_SHA256_FIXED_SIGNING, pkcs8.as_ref()).unwrap();
        let fake = FakeOidc {
            inner: Arc::new(Inner {
                key,
                rng,
                next: Mutex::default(),
                codes: Mutex::default(),
            }),
        };
        let router = Router::new()
            .route("/{p}/.well-known/openid-configuration", get(discovery))
            .route("/{p}/authorize", get(authorize))
            .route("/{p}/token", post(token))
            .route("/{p}/jwks", get(jwks))
            .route("/{p}/userinfo", get(userinfo))
            .with_state(fake.clone());
        let listener = tokio::net::TcpListener::bind(ADDR)
            .await
            .expect("cổng 4499 phải trống cho nhà cung cấp OIDC giả");
        tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
        fake
    }

    /// Đặt người dùng cho lần đăng nhập tới ở nhà cung cấp `provider`.
    pub fn next_user(&self, provider: &str, claims: Value) {
        self.inner
            .next
            .lock()
            .unwrap()
            .insert(provider.to_owned(), claims);
    }

    fn sign(&self, claims: &Value) -> String {
        let header =
            URL_SAFE_NO_PAD.encode(json!({"alg": "ES256", "typ": "JWT", "kid": "e2e"}).to_string());
        let payload = URL_SAFE_NO_PAD.encode(claims.to_string());
        let input = format!("{header}.{payload}");
        let sig = self
            .inner
            .key
            .sign(&self.inner.rng, input.as_bytes())
            .unwrap();
        format!("{input}.{}", URL_SAFE_NO_PAD.encode(sig.as_ref()))
    }
}

fn issuer(p: &str) -> String {
    format!("http://{ADDR}/{p}")
}

async fn discovery(Path(p): Path<String>) -> Json<Value> {
    let iss = issuer(&p);
    Json(json!({
        "issuer": iss,
        "authorization_endpoint": format!("{iss}/authorize"),
        "token_endpoint": format!("{iss}/token"),
        "jwks_uri": format!("{iss}/jwks"),
        "userinfo_endpoint": format!("{iss}/userinfo"),
        "response_types_supported": ["code"],
        "subject_types_supported": ["public"],
        "id_token_signing_alg_values_supported": ["ES256"],
        "token_endpoint_auth_methods_supported": ["client_secret_basic", "client_secret_post"],
    }))
}

async fn authorize(
    State(fake): State<FakeOidc>,
    Path(p): Path<String>,
    Query(q): Query<HashMap<String, String>>,
) -> Response {
    let Some(claims) = fake.inner.next.lock().unwrap().get(&p).cloned() else {
        return (StatusCode::BAD_REQUEST, "chưa đặt người dùng").into_response();
    };
    let code = uuid::Uuid::new_v4().simple().to_string();
    fake.inner
        .codes
        .lock()
        .unwrap()
        .insert(code.clone(), (p, claims, q.get("nonce").cloned()));
    let mut to = url::Url::parse(&q["redirect_uri"]).unwrap();
    to.query_pairs_mut().append_pair("code", &code).append_pair(
        "state",
        q.get("state").map(String::as_str).unwrap_or_default(),
    );
    Redirect::to(to.as_str()).into_response()
}

/// `client_id` từ Basic auth hoặc thân yêu cầu.
fn client_id(headers: &HeaderMap, form: &HashMap<String, String>) -> String {
    if let Some(basic) = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Basic "))
    {
        let raw = base64::engine::general_purpose::STANDARD
            .decode(basic)
            .unwrap_or_default();
        let raw = String::from_utf8_lossy(&raw);
        let id = raw.split(':').next().unwrap_or_default();
        return form_urlencoded::parse(id.as_bytes())
            .next()
            .map(|(k, _)| k.into_owned())
            .unwrap_or_default();
    }
    form.get("client_id").cloned().unwrap_or_default()
}

async fn token(
    State(fake): State<FakeOidc>,
    Path(p): Path<String>,
    headers: HeaderMap,
    Form(form): Form<HashMap<String, String>>,
) -> Response {
    let code = form.get("code").cloned().unwrap_or_default();
    let Some((provider, claims, nonce)) = fake.inner.codes.lock().unwrap().remove(&code) else {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "invalid_grant"})),
        )
            .into_response();
    };
    assert_eq!(provider, p);
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let mut id = json!({
        "iss": issuer(&p),
        "aud": client_id(&headers, &form),
        "iat": now,
        "exp": now + 600,
    });
    for (k, v) in claims.as_object().unwrap() {
        id[k] = v.clone();
    }
    if let Some(n) = nonce {
        id["nonce"] = json!(n);
    }
    let access = format!("at-{code}");
    fake.inner
        .codes
        .lock()
        .unwrap()
        .insert(access.clone(), (p, claims, None));
    Json(json!({
        "access_token": access,
        "token_type": "Bearer",
        "expires_in": 600,
        "id_token": fake.sign(&id),
    }))
    .into_response()
}

async fn jwks(State(fake): State<FakeOidc>) -> Json<Value> {
    // Khóa công khai P-256 dạng không nén: 0x04 ‖ x (32 byte) ‖ y (32 byte).
    let point = fake.inner.key.public_key().as_ref();
    Json(json!({"keys": [{
        "kty": "EC", "crv": "P-256", "alg": "ES256", "use": "sig", "kid": "e2e",
        "x": URL_SAFE_NO_PAD.encode(&point[1..33]),
        "y": URL_SAFE_NO_PAD.encode(&point[33..65]),
    }]}))
}

async fn userinfo(State(fake): State<FakeOidc>, headers: HeaderMap) -> Response {
    let token = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .unwrap_or_default()
        .to_owned();
    match fake.inner.codes.lock().unwrap().get(&token) {
        Some((_, claims, _)) => Json(claims.clone()).into_response(),
        None => StatusCode::UNAUTHORIZED.into_response(),
    }
}
