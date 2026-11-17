/// Đăng nhập bằng tài khoản F-Eco (spec mobile-launcher, "Đăng nhập bằng tài khoản F-Eco trên
/// điện thoại"): authorization code + PKCE qua trình duyệt hệ thống bằng AppAuth. Refresh token
/// nằm trong Keychain/Keystore; access token chỉ giữ trong bộ nhớ.
library;

import 'package:flutter_appauth/flutter_appauth.dart';
import 'package:flutter_secure_storage/flutter_secure_storage.dart';

import 'config.dart';

/// Token nhận được từ máy chủ OAuth.
class Tokens {
  const Tokens({
    required this.accessToken,
    required this.refreshToken,
    required this.expiresAt,
  });

  final String accessToken;
  final String? refreshToken;
  final DateTime expiresAt;
}

/// Nơi lưu refresh token.
abstract class TokenStore {
  Future<String?> read();
  Future<void> write(String? refreshToken);
}

class SecureTokenStore implements TokenStore {
  static const _key = 'feco.refresh_token';
  final _storage = const FlutterSecureStorage();

  @override
  Future<String?> read() => _storage.read(key: _key);

  @override
  Future<void> write(String? refreshToken) => refreshToken == null
      ? _storage.delete(key: _key)
      : _storage.write(key: _key, value: refreshToken);
}

class MemoryTokenStore implements TokenStore {
  MemoryTokenStore([this.value]);
  String? value;

  @override
  Future<String?> read() async => value;

  @override
  Future<void> write(String? refreshToken) async => value = refreshToken;
}

/// Giao tiếp với máy chủ OAuth (AppAuth trên máy thật; bản giả trong test).
abstract class TokenClient {
  /// Mở trình duyệt hệ thống để người dùng đăng nhập; trả token khi xong.
  Future<Tokens> authorize();

  /// Đổi refresh token lấy token mới (refresh token xoay vòng).
  Future<Tokens> refresh(String refreshToken);
}

class AppAuthClient implements TokenClient {
  AppAuthClient(this.config);

  final AppConfig config;
  final _appAuth = const FlutterAppAuth();
  static const _scopes = ['openid', 'offline_access', 'profile'];

  String get _issuer {
    final s = config.issuer.toString();
    return s.endsWith('/') ? s.substring(0, s.length - 1) : s;
  }

  Tokens _from(TokenResponse r) => Tokens(
    accessToken: r.accessToken ?? '',
    refreshToken: r.refreshToken,
    expiresAt:
        r.accessTokenExpirationDateTime ??
        DateTime.now().add(const Duration(minutes: 5)),
  );

  @override
  Future<Tokens> authorize() async {
    final r = await _appAuth.authorizeAndExchangeCode(
      AuthorizationTokenRequest(
        config.clientId,
        AppConfig.redirectUrl,
        issuer: _issuer,
        scopes: _scopes,
        allowInsecureConnections: config.insecureDev,
      ),
    );
    return _from(r);
  }

  @override
  Future<Tokens> refresh(String refreshToken) async {
    final r = await _appAuth.token(
      TokenRequest(
        config.clientId,
        AppConfig.redirectUrl,
        issuer: _issuer,
        refreshToken: refreshToken,
        grantType: GrantType.refreshToken,
        scopes: _scopes,
        allowInsecureConnections: config.insecureDev,
      ),
    );
    return _from(r);
  }
}

/// Phiên đăng nhập của app.
abstract class Auth {
  bool get signedIn;

  /// Khôi phục phiên từ refresh token đã lưu (lúc mở app).
  Future<bool> restore();
  Future<void> signIn();
  Future<void> signOut();

  /// Access token còn hạn; [refresh] buộc làm mới (khi API trả 401). Null khi chưa đăng nhập
  /// hoặc làm mới thất bại.
  Future<String?> accessToken({bool refresh = false});
}

class OidcAuth implements Auth {
  OidcAuth(this.client, this.store, {DateTime Function()? now})
    : _now = now ?? DateTime.now;

  final TokenClient client;
  final TokenStore store;
  final DateTime Function() _now;
  Tokens? _tokens;

  @override
  bool get signedIn => _tokens != null;

  @override
  Future<bool> restore() async => await accessToken(refresh: true) != null;

  @override
  Future<void> signIn() async {
    final t = await client.authorize();
    _tokens = t;
    await store.write(t.refreshToken);
  }

  @override
  Future<void> signOut() async {
    _tokens = null;
    await store.write(null);
  }

  @override
  Future<String?> accessToken({bool refresh = false}) async {
    final t = _tokens;
    final fresh =
        t != null &&
        t.expiresAt.isAfter(_now().add(const Duration(seconds: 30)));
    if (fresh && !refresh) return t.accessToken;
    final rt = t?.refreshToken ?? await store.read();
    if (rt == null) {
      _tokens = null;
      return null;
    }
    try {
      final n = await client.refresh(rt);
      _tokens = Tokens(
        accessToken: n.accessToken,
        refreshToken: n.refreshToken ?? rt,
        expiresAt: n.expiresAt,
      );
      await store.write(_tokens!.refreshToken);
      return n.accessToken;
    } on Exception {
      // Refresh token hết hạn hoặc bị thu hồi: về trạng thái chưa đăng nhập.
      await signOut();
      return null;
    }
  }
}
