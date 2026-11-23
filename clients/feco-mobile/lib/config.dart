/// Cấu hình đặt lúc dựng bằng `--dart-define` (design M3 của thay đổi mobile-launcher).
library;

/// Phiên bản app, trùng `version` trong pubspec.yaml (test `app_version_matches_pubspec`).
const appVersion = '1.20.0';

class AppConfig {
  const AppConfig({
    required this.server,
    required this.issuer,
    required this.clientId,
    required this.demo,
  });

  /// Máy chủ F-Eco, có `/` ở cuối. `10.0.2.2` là máy chủ của Android emulator.
  final Uri server;

  /// Máy chủ OAuth 2.0 / OIDC (Hydra).
  final Uri issuer;

  /// Client OAuth của F-Launcher di động.
  final String clientId;

  /// Dữ liệu mẫu, không gọi mạng (integration test, xem giao diện).
  final bool demo;

  /// Địa chỉ chuyển hướng OAuth: scheme của app (`vn.feco.launcher`).
  static const redirectUrl = 'vn.feco.launcher:/oauth2redirect';

  static AppConfig fromEnvironment() => AppConfig(
    server: _withSlash(
      const String.fromEnvironment(
        'FECO_SERVER',
        defaultValue: 'http://10.0.2.2:8080/',
      ),
    ),
    issuer: _withSlash(
      const String.fromEnvironment(
        'FECO_ISSUER',
        defaultValue: 'http://10.0.2.2:4444/',
      ),
    ),
    clientId: const String.fromEnvironment(
      'FECO_CLIENT_ID',
      defaultValue: 'feco-mobile',
    ),
    demo: const bool.fromEnvironment('FECO_DEMO'),
  );

  /// Máy chủ chạy bằng HTTP (chỉ dev): AppAuth cần được phép kết nối không mã hóa.
  bool get insecureDev => issuer.scheme == 'http';

  /// Địa chỉ trang web của F-Eco (ví dụ `wallet`, `library`).
  Uri page(String path) =>
      server.resolve(path.startsWith('/') ? path.substring(1) : path);

  static Uri _withSlash(String raw) =>
      Uri.parse(raw.endsWith('/') ? raw : '$raw/');
}
