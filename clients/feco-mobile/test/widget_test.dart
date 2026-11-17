// Widget test các scenario của spec mobile-launcher, với dữ liệu mẫu (không gọi mạng).

import 'package:feco_mobile/app_state.dart';
import 'package:feco_mobile/config.dart';
import 'package:feco_mobile/demo.dart';
import 'package:feco_mobile/i18n.dart';
import 'package:feco_mobile/ui.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:material_ui/material_ui.dart';

/// Cuộn tới rồi bấm (nút có thể nằm dưới thanh tab).
Future<void> tapKey(WidgetTester tester, String key) async {
  final f = find.byKey(Key(key));
  await tester.ensureVisible(f);
  await tester.pumpAndSettle();
  await tester.tap(f);
  await tester.pumpAndSettle();
}

class Harness {
  Harness() {
    state = AppState(
      config: AppConfig(
        server: Uri.parse('https://f-eco.example/'),
        issuer: Uri.parse('https://auth.f-eco.example/'),
        clientId: 'feco-mobile',
        demo: true,
      ),
      auth: DemoAuth(),
      backend: backend,
      openUrl: (u) async {
        opened.add(u);
        return true;
      },
    );
  }

  final backend = DemoBackend(balance: 10);
  final opened = <Uri>[];
  late final AppState state;

  Future<void> pump(WidgetTester tester) async {
    tester.view.physicalSize = const Size(1170, 2532);
    tester.view.devicePixelRatio = 3;
    addTearDown(tester.view.reset);
    await tester.pumpWidget(FecoApp(state: state));
    await state.start();
    await tester.pumpAndSettle();
  }
}

void main() {
  testWidgets('scenario_chua_dang_nhap', (tester) async {
    final h = Harness();
    await h.pump(tester);
    // Năm tab; Cửa hàng xem được khi chưa đăng nhập.
    for (final k in [
      'library',
      'store',
      'wallet',
      'notifications',
      'account',
    ]) {
      expect(find.byKey(Key('tab-$k')), findsOneWidget);
    }
    expect(find.text('ThesisAI'), findsOneWidget);
    await tester.tap(find.byKey(const Key('tab-wallet')));
    await tester.pumpAndSettle();
    expect(find.byKey(const Key('sign-in')), findsWidgets);
  });

  testWidgets('scenario_mo_app_web_va_app_desktop', (tester) async {
    final h = Harness();
    await h.pump(tester);
    await tapKey(tester, 'open-thesisai');
    expect(h.opened.single, Uri.parse('https://thesisai.example/auth/feco'));
    // App desktop: không có nút cài, ghi dùng trên máy tính.
    expect(find.byKey(const Key('desktop-kibi')), findsOneWidget);
    expect(find.text('Dùng trên máy tính qua F-Launcher'), findsOneWidget);
    expect(find.byKey(const Key('open-kibi')), findsNothing);
  });

  testWidgets('scenario_khong_du_f_coin', (tester) async {
    final h = Harness();
    await h.pump(tester);
    await h.state.signIn();
    await tester.pumpAndSettle();
    await tapKey(tester, 'buy-so-tay-pro');
    expect(find.text('Còn thiếu 39 F-Coin'), findsOneWidget);
    await tapKey(tester, 'topup');
    expect(h.opened.single, Uri.parse('https://f-eco.example/wallet'));
    // Đủ F-Coin thì mua được, số dư giảm.
    h.backend.balance = 100;
    await h.state.reload();
    await tester.pumpAndSettle();
    await tapKey(tester, 'buy-so-tay-pro');
    expect(
      find.text('Giá 49 F-Coin. Số dư sau khi mua: 51 F-Coin.'),
      findsOneWidget,
    );
    await tapKey(tester, 'confirm-buy');
    expect(h.state.wallet!.balance, 51);
    expect(h.state.owns('so-tay-pro'), isTrue);
  });

  testWidgets('scenario_them_app_web', (tester) async {
    final h = Harness();
    await h.pump(tester);
    await h.state.signIn();
    await tester.pumpAndSettle();
    await tapKey(tester, 'add-thesisai');
    expect(find.byKey(const Key('add-thesisai')), findsNothing);
    await tester.tap(find.byKey(const Key('tab-library')));
    await tester.pumpAndSettle();
    expect(find.byKey(const Key('library-thesisai')), findsOneWidget);
  });

  testWidgets('scenario_doi_ngon_ngu', (tester) async {
    final h = Harness();
    await h.pump(tester);
    await tester.tap(find.byKey(const Key('tab-account')));
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const Key('lang-ja')));
    await tester.pumpAndSettle();
    expect(h.state.lang, Lang.ja);
    expect(find.text('ストア'), findsOneWidget);
    expect(find.text('アカウント'), findsWidgets);
  });
}
