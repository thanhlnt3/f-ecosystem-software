// Widget test các scenario của spec mobile-launcher, với dữ liệu mẫu (không gọi mạng).

import 'package:feco_mobile/api.dart';
import 'package:feco_mobile/app_state.dart';
import 'package:feco_mobile/config.dart';
import 'package:feco_mobile/demo.dart';
import 'package:feco_mobile/i18n.dart';
import 'package:feco_mobile/models.dart';
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
    // Sáu tab; Cửa hàng xem được khi chưa đăng nhập.
    for (final k in [
      'library',
      'store',
      'wallet',
      'messages',
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

  // app-update-notices: "App máy tính có bản mới", "App web", "Quay lại app".
  testWidgets('scenario_bao_ban_moi_tren_dien_thoai', (tester) async {
    final h = Harness();
    await h.backend.addToLibrary('kibi', '0.3.0');
    await h.backend.addToLibrary('thesisai', '0.0.9');
    await h.pump(tester);
    await h.state.signIn();
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const Key('tab-library')));
    await tester.pumpAndSettle();
    // App máy tính có bản mới: nhãn, dòng tóm tắt, huy hiệu 1.
    expect(find.byKey(const Key('update-kibi')), findsOneWidget);
    expect(
      find.text(
        'Số app có bản mới: 1. Mở F-Launcher trên máy tính để cập nhật.',
      ),
      findsOneWidget,
    );
    expect(
      find.descendant(
        of: find.byKey(const Key('tab-library')),
        matching: find.text('1'),
      ),
      findsOneWidget,
    );
    // App web: không có nhãn.
    expect(find.byKey(const Key('library-thesisai')), findsOneWidget);
    expect(find.byKey(const Key('update-thesisai')), findsNothing);
    // Cập nhật trên máy tính rồi quay lại app: tải lại, hết "Có bản mới".
    await h.backend.addToLibrary('kibi', '0.3.3');
    tester.binding.handleAppLifecycleStateChanged(AppLifecycleState.inactive);
    tester.binding.handleAppLifecycleStateChanged(AppLifecycleState.hidden);
    tester.binding.handleAppLifecycleStateChanged(AppLifecycleState.paused);
    tester.binding.handleAppLifecycleStateChanged(AppLifecycleState.hidden);
    tester.binding.handleAppLifecycleStateChanged(AppLifecycleState.inactive);
    tester.binding.handleAppLifecycleStateChanged(AppLifecycleState.resumed);
    await tester.pumpAndSettle();
    expect(find.byKey(const Key('update-kibi')), findsNothing);
    expect(find.byKey(const Key('updates-banner')), findsNothing);
    expect(h.state.updateCount, 0);
  });

  // admin-list-tools: "Quản trị viên mở báo cáo tin nhắn", "Người dùng thường".
  testWidgets('scenario_bang_quan_tri_tren_dien_thoai', (tester) async {
    final h = Harness();
    h.backend.role = 'admin';
    h.backend.adminSections = const [
      AdminSection('reviews', 'https://f-eco.example/admin/reviews'),
      AdminSection('chat-reports', 'https://f-eco.example/admin/chat-reports'),
    ];
    await h.pump(tester);
    await h.state.signIn();
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const Key('tab-account')));
    await tester.pumpAndSettle();
    expect(find.byKey(const Key('admin-console-card')), findsOneWidget);
    expect(find.text('Hàng chờ duyệt app'), findsOneWidget);
    await tapKey(tester, 'admin-chat-reports');
    expect(h.opened, [Uri.parse('https://f-eco.example/admin/chat-reports')]);
  });

  testWidgets('scenario_nguoi_dung_thuong_khong_co_bang_quan_tri', (
    tester,
  ) async {
    final h = Harness();
    await h.pump(tester);
    await h.state.signIn();
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const Key('tab-account')));
    await tester.pumpAndSettle();
    expect(find.byKey(const Key('admin-console-card')), findsNothing);
  });

  // distribution-upkeep-and-real-email: "Máy chủ trả lỗi", "Quay lại sau lỗi".
  testWidgets('scenario_loi_may_chu_va_quay_lai', (tester) async {
    final h = Harness();
    h.backend.failure = const ApiException(503, 'unavailable', 'Down', 'Lỗi');
    await h.pump(tester);
    expect(
      h.state.error,
      'Máy chủ F-Eco đang gặp lỗi. Hãy thử lại sau ít phút.',
    );
    expect(find.text('Không kết nối được máy chủ F-Eco.'), findsNothing);
    // Mất mạng thật thì vẫn báo mất kết nối.
    h.backend.failure = Exception('socket');
    await h.state.reload();
    expect(h.state.error, 'Không kết nối được máy chủ F-Eco.');
    // Máy chủ ổn lại, người dùng quay lại app: tự tải lại, hết lỗi.
    h.backend.failure = null;
    tester.binding.handleAppLifecycleStateChanged(AppLifecycleState.inactive);
    tester.binding.handleAppLifecycleStateChanged(AppLifecycleState.hidden);
    tester.binding.handleAppLifecycleStateChanged(AppLifecycleState.paused);
    tester.binding.handleAppLifecycleStateChanged(AppLifecycleState.hidden);
    tester.binding.handleAppLifecycleStateChanged(AppLifecycleState.inactive);
    tester.binding.handleAppLifecycleStateChanged(AppLifecycleState.resumed);
    await tester.pumpAndSettle();
    expect(h.state.error, isNull);
    expect(h.state.catalog, isNotEmpty);
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
    expect(find.text('メッセージ'), findsOneWidget);
  });

  // ---- Tin nhắn (thay đổi chat-and-source-scan, spec mobile-launcher "Tin nhắn trên điện thoại").

  /// Đăng nhập, mở tab Tin nhắn rồi mở hội thoại [id].
  Future<Harness> openConversation(WidgetTester tester, String id) async {
    final h = Harness();
    await h.pump(tester);
    await h.state.signIn();
    await tester.pumpAndSettle();
    await tapKey(tester, 'tab-messages');
    await tapKey(tester, 'conv-$id');
    return h;
  }

  Finder chatBadge(String label) => find.descendant(
    of: find.byKey(const Key('tab-messages')),
    matching: find.text(label),
  );

  // Scenario "Gửi tin từ điện thoại": mở hội thoại trong tab Tin nhắn, gõ "Cảm ơn anh", bấm Gửi
  // thì tin hiện ở cuối luồng.
  testWidgets('scenario_gui_tin_tu_dien_thoai', (tester) async {
    final h = Harness();
    await h.pump(tester);
    await h.state.signIn();
    await tester.pumpAndSettle();
    // Huy hiệu số chưa đọc trên tab.
    expect(chatBadge('2'), findsOneWidget);
    await tapKey(tester, 'tab-messages');
    expect(find.text('Website bán hàng'), findsOneWidget);
    expect(
      find.byKey(const Key('conv-unread-${DemoBackend.projectChat}')),
      findsOneWidget,
    );
    await tapKey(tester, 'conv-${DemoBackend.projectChat}');
    // Mở hội thoại thì đánh dấu đã đọc tới tin cuối, huy hiệu cập nhật.
    expect(h.backend.lastRead, (DemoBackend.projectChat, 5));
    expect(h.state.chatUnread, 0);

    await tester.enterText(find.byKey(const Key('chat-input')), 'Cảm ơn anh');
    await tester.tap(find.text('Gửi'));
    await tester.pumpAndSettle();

    final sent = find.text('Cảm ơn anh');
    expect(sent, findsOneWidget, reason: 'tin trong luồng, ô soạn đã xóa');
    final previous = find.text('Được chị, em làm xong trong tuần này.');
    expect(
      tester.getTopLeft(sent).dy,
      greaterThan(tester.getBottomLeft(previous).dy),
      reason: 'tin mới ở cuối luồng',
    );
    expect(
      tester.getCenter(sent).dx,
      greaterThan(
        tester.view.physicalSize.width / tester.view.devicePixelRatio / 2,
      ),
      reason: 'tin của mình bên phải',
    );
    final stored = await h.backend.messages(DemoBackend.projectChat);
    expect((stored.last.body, stored.last.mine), ('Cảm ơn anh', true));

    await tester.pageBack();
    await tester.pumpAndSettle();
    expect(chatBadge('2'), findsNothing);
  });

  // Scenario "Chưa đăng nhập": tab Tin nhắn hiện nút "Đăng nhập".
  testWidgets('scenario_tin_nhan_chua_dang_nhap', (tester) async {
    final h = Harness();
    await h.pump(tester);
    await tapKey(tester, 'tab-messages');
    final page = find.byKey(const Key('page-messages'));
    final button = find.descendant(
      of: page,
      matching: find.byKey(const Key('sign-in')),
    );
    expect(button, findsOneWidget);
    expect(
      find.descendant(of: button, matching: find.text('Đăng nhập')),
      findsOneWidget,
    );
    expect(find.byKey(const Key('conversation-list')), findsNothing);
    await tester.tap(button);
    await tester.pumpAndSettle();
    expect(h.state.signedIn, isTrue);
    expect(
      find.byKey(const Key('conv-${DemoBackend.projectChat}')),
      findsOneWidget,
    );
  });

  testWidgets('chat_polls_every_5_seconds_while_open', (tester) async {
    final h = await openConversation(tester, DemoBackend.appChat);
    final calls = h.backend.messageCalls;
    final m = h.backend.receive(DemoBackend.appChat, 'Bạn cần gì thêm không?');
    expect(find.text('Bạn cần gì thêm không?'), findsNothing);
    // Mở hội thoại chưa tới 5 giây: lần hỏi đầu tiên trong 5 giây tới.
    await tester.pump(const Duration(seconds: 5));
    await tester.pumpAndSettle();
    expect(find.text('Bạn cần gì thêm không?'), findsOneWidget);
    expect(h.backend.messageCalls, calls + 1);
    // Có tin mới thì đánh dấu đã đọc.
    expect(h.backend.lastRead, (DemoBackend.appChat, m.id));
    // Không trùng khi hỏi lại.
    await tester.pump(const Duration(seconds: 5));
    await tester.pumpAndSettle();
    expect(find.text('Bạn cần gì thêm không?'), findsOneWidget);

    // Rời hội thoại thì ngừng hỏi.
    await tester.pageBack();
    await tester.pumpAndSettle();
    final after = h.backend.messageCalls;
    await tester.pump(const Duration(seconds: 20));
    expect(h.backend.messageCalls, after);
  });

  testWidgets('chat_client_validation_and_blocked_customer', (tester) async {
    final h = await openConversation(tester, DemoBackend.appChat);
    await tester.tap(find.byKey(const Key('chat-send')));
    await tester.pumpAndSettle();
    expect(find.text('Hãy nhập nội dung tin nhắn.'), findsOneWidget);
    await tester.enterText(find.byKey(const Key('chat-input')), 'x' * 5001);
    await tester.tap(find.byKey(const Key('chat-send')));
    await tester.pumpAndSettle();
    expect(find.byKey(const Key('chat-error')), findsOneWidget);
    expect(
      find.text(
        'Tin nhắn dài quá 5.000 ký tự. Hãy rút gọn hoặc chia thành nhiều tin.',
      ),
      findsOneWidget,
    );
    await tester.pageBack();
    await tester.pumpAndSettle();

    // Bị nhà phát triển chặn: ô soạn tắt, có giải thích.
    await tapKey(tester, 'conv-${DemoBackend.blockedChat}');
    expect(find.byKey(const Key('chat-blocked')), findsOneWidget);
    final field = tester.widget<TextField>(find.byKey(const Key('chat-input')));
    expect(field.enabled, isFalse);
    final send = tester.widget<FilledButton>(
      find.byKey(const Key('chat-send')),
    );
    expect(send.onPressed, isNull);
    // Không gửi gì vào hội thoại bị chặn.
    expect((await h.backend.messages(DemoBackend.blockedChat)).length, 1);
  });

  testWidgets('chat_attachments_are_view_only', (tester) async {
    final h = await openConversation(tester, DemoBackend.projectChat);
    // Tệp sạch: mở bằng trình duyệt.
    expect(find.text('471 KB · Đã kiểm tra, an toàn'), findsOneWidget);
    await tapKey(tester, 'att-open-2');
    expect(
      h.opened.last,
      Uri.parse(
        'https://f-eco.example/messages/${DemoBackend.projectChat}/files/a1',
      ),
    );
    // Đang quét: không mở được.
    expect(find.text('118 KB · Đang kiểm tra'), findsOneWidget);
    expect(find.byKey(const Key('att-open-5')), findsNothing);
    // Gửi tệp qua web.
    await tapKey(tester, 'chat-attach-web');
    expect(
      h.opened.last,
      Uri.parse('https://f-eco.example/messages/${DemoBackend.projectChat}'),
    );
    // Tin quản trị viên và tin hệ thống.
    expect(find.text('Quản trị F-Eco'), findsOneWidget);
    await tester.pageBack();
    await tester.pumpAndSettle();

    // Có mã độc: chặn, không mở.
    await tapKey(tester, 'conv-${DemoBackend.appChat}');
    expect(find.text('2 KB · Có mã độc, đã chặn'), findsOneWidget);
    expect(find.byKey(const Key('att-open-7')), findsNothing);
  });
}
