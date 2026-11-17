// Integration test chạy trên Android emulator (API 29, 36) và iOS Simulator trong CI (spec
// mobile-launcher, scenario "Android thấp nhất và mới nhất", "iOS"). Dùng dữ liệu mẫu, không gọi
// mạng.

import 'package:feco_mobile/config.dart';
import 'package:feco_mobile/i18n.dart';
import 'package:feco_mobile/main.dart';
import 'package:feco_mobile/ui.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:integration_test/integration_test.dart';
import 'package:material_ui/material_ui.dart';

Future<void> tapKey(WidgetTester tester, String key) async {
  final f = find.byKey(Key(key));
  await tester.ensureVisible(f);
  await tester.pumpAndSettle();
  await tester.tap(f);
  await tester.pumpAndSettle();
}

void main() {
  IntegrationTestWidgetsFlutterBinding.ensureInitialized();

  testWidgets('app_runs_on_device', (tester) async {
    final opened = <Uri>[];
    final state = buildState(
      AppConfig(
        server: Uri.parse('https://f-eco.example/'),
        issuer: Uri.parse('https://auth.f-eco.example/'),
        clientId: 'feco-mobile',
        demo: true,
      ),
      openUrl: (u) async {
        opened.add(u);
        return true;
      },
    );
    // Kiểm bằng tiếng Việt, dù máy dùng ngôn ngữ nào.
    state.lang = Lang.vi;
    await tester.pumpWidget(FecoApp(state: state));
    await state.start();
    await tester.pumpAndSettle();

    // Năm tab.
    for (final k in [
      'library',
      'store',
      'wallet',
      'notifications',
      'account',
    ]) {
      expect(find.byKey(Key('tab-$k')), findsOneWidget);
    }
    // Cửa hàng: app web mở bằng trình duyệt; app desktop không có nút cài.
    expect(find.byKey(const Key('desktop-kibi')), findsOneWidget);
    await tapKey(tester, 'open-thesisai');
    expect(opened.last.host, 'thesisai.example');

    // Đăng nhập (dữ liệu mẫu), mua khi thiếu F-Coin.
    await tapKey(tester, 'tab-wallet');
    await tapKey(tester, 'sign-in');
    expect(find.byKey(const Key('wallet-balance')), findsOneWidget);
    await tapKey(tester, 'tab-store');
    await tapKey(tester, 'buy-so-tay-pro');
    expect(find.byKey(const Key('topup')), findsOneWidget);
    await tapKey(tester, 'topup');
    expect(opened.last.path, '/wallet');

    // Thêm app web vào Ứng dụng của tôi.
    await tapKey(tester, 'add-thesisai');
    await tapKey(tester, 'tab-library');
    expect(find.byKey(const Key('library-thesisai')), findsOneWidget);

    // Đổi ngôn ngữ.
    await tapKey(tester, 'tab-account');
    await tapKey(tester, 'lang-ja');
    expect(find.text('ストア'), findsOneWidget);
  });
}
