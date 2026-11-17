/// F-Launcher di động (thay đổi OpenSpec mobile-launcher): ứng dụng đồng hành của F-Eco trên
/// Android 10+ và iOS 18+.
library;

import 'dart:ui' as ui;

import 'package:material_ui/material_ui.dart';
import 'package:url_launcher/url_launcher.dart';

import 'api.dart';
import 'app_state.dart';
import 'auth.dart';
import 'config.dart';
import 'demo.dart';
import 'i18n.dart';
import 'ui.dart';

/// Dựng trạng thái của app theo cấu hình (bản thật hoặc dữ liệu mẫu).
AppState buildState(AppConfig config, {UrlOpener? openUrl}) {
  final Auth auth;
  final Backend backend;
  if (config.demo) {
    auth = DemoAuth();
    backend = DemoBackend();
  } else {
    auth = OidcAuth(AppAuthClient(config), SecureTokenStore());
    backend = HttpBackend(config, auth);
  }
  return AppState(
    config: config,
    auth: auth,
    backend: backend,
    openUrl:
        openUrl ??
        (url) => launchUrl(url, mode: LaunchMode.externalApplication),
    lang: Lang.fromCode(ui.PlatformDispatcher.instance.locale.languageCode),
  );
}

Future<void> main() async {
  WidgetsFlutterBinding.ensureInitialized();
  final state = buildState(AppConfig.fromEnvironment());
  runApp(FecoApp(state: state));
  await state.start();
}
