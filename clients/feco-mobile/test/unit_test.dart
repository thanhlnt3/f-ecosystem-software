// Unit test của F-Launcher di động (thay đổi mobile-launcher, design M5).

import 'dart:convert';
import 'dart:io';

import 'package:feco_mobile/api.dart';
import 'package:feco_mobile/auth.dart';
import 'package:feco_mobile/chat.dart';
import 'package:feco_mobile/config.dart';
import 'package:feco_mobile/demo.dart';
import 'package:feco_mobile/i18n.dart' hide group;
import 'package:feco_mobile/i18n.dart' as fmt show group;
import 'package:feco_mobile/models.dart';
import 'package:feco_mobile/web_url.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:http/http.dart' as http;
import 'package:http/testing.dart';

class FakeTokenClient implements TokenClient {
  int refreshes = 0;
  bool failRefresh = false;

  @override
  Future<Tokens> authorize() async => Tokens(
    accessToken: 'a0',
    refreshToken: 'r0',
    expiresAt: DateTime.now().add(const Duration(hours: 1)),
  );

  @override
  Future<Tokens> refresh(String refreshToken) async {
    if (failRefresh) throw Exception('invalid_grant');
    refreshes++;
    return Tokens(
      accessToken: 'a$refreshes',
      refreshToken: 'r$refreshes',
      expiresAt: DateTime(2026, 11, 17, 11),
    );
  }
}

AppConfig cfg() => AppConfig(
  server: Uri.parse('https://f-eco.example/'),
  issuer: Uri.parse('https://auth.f-eco.example/'),
  clientId: 'feco-mobile',
  demo: false,
);

void main() {
  test('app_version_matches_pubspec', () {
    final pubspec = File('pubspec.yaml').readAsStringSync();
    final version = RegExp(r'^version: ([0-9.]+)\+', multiLine: true).firstMatch(pubspec)!.group(1);
    expect(appVersion, version);
  });

  test('every_key_has_four_languages', () {
    for (final e in strings.entries) {
      expect(e.value.length, 4, reason: e.key);
      for (final s in e.value) {
        expect(s.trim(), isNotEmpty, reason: e.key);
      }
    }
    expect(tr(Lang.ja, 'tab.store'), 'ストア');
    expect(tr(Lang.vi, 'buy', {'n': 49}), 'Mua · 49 F-Coin');
    expect(fmt.group(1234567, Lang.vi), '1.234.567');
    expect(fmt.group(1000, Lang.en), '1,000');
  });

  test('web_urls_are_https_or_dev_hosts', () {
    expect(validWebUrl('https://thesisai.example/auth/feco'), isTrue);
    expect(validWebUrl('http://localhost:3000/auth/feco'), isTrue);
    expect(validWebUrl('http://10.0.2.2:3000/'), isTrue);
    for (final bad in [
      'http://example.com/app',
      'javascript:alert(1)',
      'file:///etc/passwd',
      'https://user:pass@example.com/',
      '',
      null,
    ]) {
      expect(validWebUrl(bad), isFalse, reason: '$bad');
    }
    expect(
      forDevice(
        Uri.parse('http://localhost:3000/auth/feco'),
        Uri.parse('http://10.0.2.2:8080/'),
      ),
      Uri.parse('http://10.0.2.2:3000/auth/feco'),
    );
  });

  test('scenario_khong_du_f_coin_shortfall', () {
    expect(shortfall(10, 49), 39);
    expect(shortfall(100, 49), 0);
  });

  test('json_from_the_server', () {
    final apps = CatalogApp.listFromJson(
      jsonDecode(
        '''
      {"apps":[{"id":"thesisai","name":"ThesisAI","developer":"x","summary":"s","category":"education",
        "price_coin":0,"has_icon":true,"web_url":"http://localhost:3000/auth/feco","platforms":["web"],
        "latest_version":"0.1.0"},
       {"id":"kibi","name":"kibi","developer":"y","summary":"","category":"tools","price_coin":0,
        "has_icon":false,"web_url":null,"platforms":["windows-x64"],"latest_version":"0.3.3"}]}''',
      ) as Map<String, Object?>,
    );
    expect(apps.map((a) => a.isWeb), [true, false]);
    final w = Wallet.fromJson(
      jsonDecode(
        '{"balance":10,"rate_vnd":1000,"frozen":false,"owned":["a"],"prices":{"b":49},"wallet_url":"https://f/wallet"}',
      ) as Map<String, Object?>,
    );
    expect((w.balance, w.owns('a'), w.prices['b']), (10, true, 49));
    final lib = LibraryItem.listFromJson(
      jsonDecode(
        '{"apps":[{"id":"thesisai","name":"ThesisAI","has_icon":false,"installed":true,"installed_version":"0.1.0","purchased":false,"latest":"0.2.0","web_url":"https://t/","update_available":true}]}',
      ) as Map<String, Object?>,
    );
    expect(
      (lib.single.updateAvailable, lib.single.webUrl),
      (true, 'https://t/'),
    );
    final notes = NotificationItem.listFromJson(
      jsonDecode(
        '{"notifications":[{"id":1,"title":"cũ","body":"","link":"/","created_at":"2026-11-01T00:00:00Z","read":true},{"id":2,"title":"mới","body":"","link":"/wallet","created_at":"2026-11-17T00:00:00Z","read":false}]}',
      ) as Map<String, Object?>,
    );
    expect(notes.first.title, 'mới', reason: 'mới nhất trước');
  });

  // Scenario "Token hết hạn": làm mới bằng refresh token, lưu refresh token mới; làm mới thất bại
  // thì về chưa đăng nhập.
  test('scenario_token_het_han', () async {
    final client = FakeTokenClient();
    final store = MemoryTokenStore('r-saved');
    var now = DateTime(2026, 11, 17, 9);
    final auth = OidcAuth(client, store, now: () => now);
    expect(await auth.restore(), isTrue);
    expect(store.value, 'r1');
    expect(await auth.accessToken(), 'a1');
    now = DateTime(2026, 11, 17, 12); // a1 hết hạn lúc 11:00
    expect(await auth.accessToken(), 'a2');
    expect(store.value, 'r2');
    client.failRefresh = true;
    expect(await auth.accessToken(refresh: true), isNull);
    expect(auth.signedIn, isFalse);
    expect(store.value, isNull, reason: 'xóa refresh token đã hỏng');
  });

  test('api_retries_once_after_401', () async {
    final client = FakeTokenClient();
    final auth = OidcAuth(client, MemoryTokenStore());
    await auth.signIn();
    final seen = <String>[];
    final http.Client mock = MockClient((req) async {
      seen.add(req.headers['Authorization'] ?? '');
      if (req.headers['Authorization'] == 'Bearer a0') {
        return http.Response('{"error":"invalid_token"}', 401);
      }
      return http.Response(
        '{"balance":7,"rate_vnd":1000,"frozen":false,"owned":[],"prices":{},"wallet_url":""}',
        200,
      );
    });
    final api = HttpBackend(cfg(), auth, client: mock);
    final w = await api.wallet();
    expect(w.balance, 7);
    expect(seen, ['Bearer a0', 'Bearer a1']);
  });

  test('purchase_errors_are_reported', () async {
    final auth = OidcAuth(FakeTokenClient(), MemoryTokenStore());
    await auth.signIn();
    final http.Client mock = MockClient((req) async {
      expect(jsonDecode(req.body), {
        'app_id': 'so-tay-pro',
        'expected_price': 49,
      });
      return http.Response.bytes(
        utf8.encode(
          jsonEncode({
            'error': 'insufficient_funds',
            'message': 'Not enough',
            'message_vi': 'Không đủ F-Coin: còn thiếu 39.',
          }),
        ),
        402,
        headers: {'content-type': 'application/json; charset=utf-8'},
      );
    });
    final r = await HttpBackend(
      cfg(),
      auth,
      client: mock,
    ).purchase('so-tay-pro', 49);
    expect(r, isA<PurchaseFailed>());
    expect((r as PurchaseFailed).code, 'insufficient_funds');
    expect(r.message, 'Không đủ F-Coin: còn thiếu 39.');
  });

  // ---- Tin nhắn (`/v1/chat`, design C4 của thay đổi chat-and-source-scan).

  Map<String, Object?> j(String s) => jsonDecode(s) as Map<String, Object?>;

  test('chat_conversations_json', () {
    const id = '0b6c1f9e-4c1a-4f6e-9d55-2f1d1d8f0a01';
    final list = Conversation.listFromJson(
      j('''
      {"conversations":[
        {"id":"$id","kind":"project","role":"customer","peer_name":"Minh","title":"Website",
         "project_id":"p1","app_id":null,"blocked":false,"last_message":"Chào",
         "last_message_at":"2026-11-18T09:30:00Z","unread":2,
         "web_url":"https://f-eco.example/messages/$id"},
        {"id":"c2","kind":"direct","role":"customer","peer_name":"Dev","title":"Dev",
         "project_id":null,"app_id":"mentor","blocked":true,"last_message":null,
         "last_message_at":"bad","unread":-3,"web_url":"javascript:alert(1)"},
        {"id":"../../v1/wallet","kind":"direct","role":"customer"},
        {"id":"c4","kind":"group","role":"customer"},
        {"id":"c5","kind":"direct","role":"admin"},
        {"id":7,"kind":"direct","role":"customer"},
        "rác",
        null
      ]}'''),
    );
    expect(list.map((c) => c.id), [
      id,
      'c2',
    ], reason: 'bỏ mục hỏng, giữ thứ tự');
    final a = list[0];
    expect(
      (a.kind, a.role, a.peerName, a.title),
      (ConversationKind.project, ChatRole.customer, 'Minh', 'Website'),
    );
    expect((a.projectId, a.appId, a.unread), ('p1', null, 2));
    expect(a.lastMessageAt, DateTime.utc(2026, 11, 18, 9, 30));
    expect(a.webUrl, 'https://f-eco.example/messages/$id');
    expect(a.customerBlocked, isFalse);
    final b = list[1];
    expect(
      (b.lastMessage, b.lastMessageAt, b.unread, b.webUrl),
      (null, null, 0, null),
    );
    expect(b.customerBlocked, isTrue);
    expect(Conversation.listFromJson(const {}), isEmpty);
    // Bị chặn nhưng mình là nhà phát triển, hoặc hội thoại dự án: vẫn gửi được.
    expect(
      Conversation.tryFromJson(
        j('{"id":"x","kind":"direct","role":"developer","blocked":true}'),
      )!.customerBlocked,
      isFalse,
    );
    expect(
      Conversation.tryFromJson(
        j('{"id":"x","kind":"project","role":"customer","blocked":true}'),
      )!.customerBlocked,
      isFalse,
    );
  });

  test('chat_messages_json_and_attachment_statuses', () {
    final list = ChatMessage.listFromJson(
      j('''
      {"messages":[
        {"id":45,"side":"system","mine":false,"body":"Dự án đã bàn giao","at":"2026-11-18T10:00:00Z","attachment":null},
        {"id":42,"side":"developer","mine":false,"body":"a.pdf","at":"2026-11-18T09:00:00Z",
         "attachment":{"name":"a.pdf","size":1234,"status":"clean","url":"https://f-eco.example/messages/c/files/a"}},
        {"id":43,"side":"customer","mine":true,"body":"x","at":"2026-11-18T09:01:00Z",
         "attachment":{"name":"b.zip","size":10,"status":"pending","url":"https://f-eco.example/messages/c/files/b"}},
        {"id":44,"side":"admin","mine":false,"body":"y","at":"2026-11-18T09:02:00Z",
         "attachment":{"name":"c.exe","size":10,"status":"infected","url":"https://f-eco.example/messages/c/files/c"}},
        {"id":46,"side":"developer","mine":false,"body":"z","at":"2026-11-18T09:03:00Z",
         "attachment":{"name":"d.bin","size":10,"status":"error","url":null}},
        {"id":47,"side":"developer","mine":false,"body":"w","at":"2026-11-18T09:04:00Z",
         "attachment":{"name":"e.txt","size":10,"status":"weird","url":"https://f-eco.example/f"}},
        {"id":48,"side":"developer","mine":false,"body":"v","at":"2026-11-18T09:05:00Z",
         "attachment":{"name":"f.txt","size":10,"status":"clean","url":"http://evil.example/f"}},
        {"id":42,"side":"developer","mine":false,"body":"a.pdf (trùng)","at":"2026-11-18T09:00:00Z",
         "attachment":{"name":"a.pdf","size":1234,"status":"clean","url":"https://f-eco.example/messages/c/files/a"}},
        {"id":"49","side":"developer","body":"id chuỗi"},
        {"id":0,"side":"developer","body":"id 0"},
        {"id":50,"side":"robot","body":"side lạ"},
        {"id":51,"body":"thiếu side"},
        []
      ]}'''),
    );
    expect(list.map((m) => m.id), [42, 43, 44, 45, 46, 47, 48]);
    expect(list.map((m) => m.side), [
      MessageSide.developer,
      MessageSide.customer,
      MessageSide.admin,
      MessageSide.system,
      MessageSide.developer,
      MessageSide.developer,
      MessageSide.developer,
    ]);
    expect(list[0].body, 'a.pdf (trùng)', reason: 'trùng id: giữ mục sau');
    expect(list[1].mine, isTrue);
    expect(list[3].attachment, isNull);
    expect(list[0].at, DateTime.utc(2026, 11, 18, 9));
    final atts = {for (final m in list) m.id: m.attachment};
    expect(atts[42]!.status, AttachmentStatus.clean);
    expect((atts[42]!.name, atts[42]!.size), ('a.pdf', 1234));
    expect(atts[42]!.canOpen, isTrue);
    expect(atts[43]!.status, AttachmentStatus.pending);
    expect(atts[43]!.canOpen, isFalse);
    expect(atts[44]!.status, AttachmentStatus.infected);
    expect(atts[44]!.canOpen, isFalse);
    expect(atts[46]!.status, AttachmentStatus.error);
    expect(atts[46]!.canOpen, isFalse);
    expect(atts[47]!.status, AttachmentStatus.error, reason: 'trạng thái lạ');
    expect(atts[47]!.canOpen, isFalse);
    expect(atts[48]!.url, isNull, reason: 'địa chỉ HTTP lạ bị bỏ');
    expect(atts[48]!.canOpen, isFalse);
    expect(ChatMessage.listFromJson(const {}), isEmpty);
  });

  test('chat_merge_by_id_in_order', () {
    ChatMessage m(int id, [String body = '']) => ChatMessage(
      id: id,
      side: MessageSide.developer,
      mine: false,
      body: body,
    );
    final merged = mergeMessages(
      [m(1), m(3, 'cũ'), m(2)],
      [m(5), m(3, 'mới'), m(4), m(5)],
    );
    expect(merged.map((x) => x.id), [1, 2, 3, 4, 5]);
    expect(merged[2].body, 'mới', reason: 'tin mới thay tin cùng id');
    expect(mergeMessages(const [], const []), isEmpty);
  });

  test('chat_client_validation', () {
    expect(validateChatBody(''), 'chat_empty');
    expect(validateChatBody('  \n\t '), 'chat_empty');
    expect(validateChatBody('Cảm ơn anh'), isNull);
    expect(validateChatBody('a' * 5000), isNull);
    expect(
      validateChatBody('  ${'a' * 5000}\n'),
      isNull,
      reason: 'bỏ khoảng trắng hai đầu',
    );
    expect(validateChatBody('a' * 5001), 'chat_too_long');
    // Đếm theo ký tự, không theo đơn vị UTF-16.
    expect(validateChatBody('😀' * 5000), isNull);
    expect(validateChatBody('😀' * 5001), 'chat_too_long');
  });

  test('chat_error_codes_to_messages', () {
    for (final code in [
      'chat_blocked',
      'chat_too_long',
      'chat_empty',
      'chat_rate_limited',
      'chat_forbidden',
      'chat_not_found',
      'chat_not_developer',
      'chat_self',
    ]) {
      expect(chatErrorKey(code), 'chat.err.$code');
      expect(strings.containsKey(chatErrorKey(code)), isTrue, reason: code);
    }
    expect(chatErrorKey('something_new'), 'chat.err.generic');
    expect(chatErrorKey(chatNetworkError), 'error_network');
    expect(chatErrorKey('not_signed_in'), 'need_sign_in');
    expect(
      chatErrorMessage(Lang.vi, 'chat_blocked'),
      'Bạn không gửi được tin vì nhà phát triển đã chặn hội thoại này.',
    );
    expect(
      chatErrorMessage(Lang.en, 'chat_rate_limited'),
      'You’re sending too fast. Wait a moment and try again.',
    );
    expect(
      chatErrorCode(const ApiException(409, 'chat_blocked', '', '')),
      'chat_blocked',
    );
    expect(chatErrorCode(const FormatException('x')), chatNetworkError);
  });

  test('chat_time_and_size_formats', () {
    final now = DateTime(2026, 11, 19, 15);
    expect(chatTime(DateTime(2026, 11, 19, 9, 5), Lang.vi, now: now), '09:05');
    expect(
      chatTime(DateTime(2026, 11, 2, 9, 5), Lang.vi, now: now),
      '02/11 09:05',
    );
    expect(
      chatTime(DateTime(2026, 11, 2, 9, 5), Lang.en, now: now),
      '11/2 09:05',
    );
    expect(
      chatTime(DateTime(2025, 1, 2, 9, 5), Lang.fr, now: now),
      '02/01/2025 09:05',
    );
    expect(chatTime(null, Lang.vi), '');
    expect(chatFileSize(900, Lang.vi), '900 B');
    expect(chatFileSize(1234, Lang.en), '2 KB');
    expect(chatFileSize(5 * 1024 * 1024 + 300000, Lang.vi), '5,3 MB');
    expect(chatFileSize(5 * 1024 * 1024 + 300000, Lang.fr), '5,3 Mo');
    expect(chatFileSize(5 * 1024 * 1024 + 300000, Lang.ja), '5.3 MB');
  });

  test('chat_http_backend_paths_and_errors', () async {
    final auth = OidcAuth(FakeTokenClient(), MemoryTokenStore());
    await auth.signIn();
    const id = '0b6c1f9e-4c1a-4f6e-9d55-2f1d1d8f0a01';
    final seen = <String>[];
    final bodies = <Object?>[];
    final http.Client mock = MockClient((req) async {
      seen.add('${req.method} ${req.url}');
      bodies.add(req.body.isEmpty ? null : jsonDecode(req.body));
      expect(req.headers['Authorization'], 'Bearer a0');
      final path = req.url.path;
      if (path == '/v1/chat/unread') {
        return http.Response('{"unread":3}', 200);
      }
      if (path == '/v1/chat/conversations') {
        return http.Response(
          '{"conversations":[{"id":"$id","kind":"direct","role":"customer",'
          '"peer_name":"Dev","title":"Dev","blocked":false,"unread":1,'
          '"last_message_at":"2026-11-18T09:00:00Z","web_url":"https://f-eco.example/messages/$id"}]}',
          200,
        );
      }
      if (path == '/v1/chat/conversations/$id/messages' &&
          req.method == 'GET') {
        return http.Response(
          '{"messages":[{"id":43,"side":"developer","mine":false,"body":"b","at":"2026-11-18T09:00:00Z","attachment":null},'
          '{"id":42,"side":"developer","mine":false,"body":"a","at":"2026-11-18T09:00:00Z","attachment":null}]}',
          200,
        );
      }
      if (path == '/v1/chat/conversations/$id/messages') {
        final body = (jsonDecode(req.body) as Map)['body'];
        if (body == 'bị chặn') {
          return http.Response('{"error":"chat_blocked"}', 409);
        }
        if (body == 'nhanh') {
          return http.Response('{"error":"chat_rate_limited"}', 429);
        }
        return http.Response.bytes(
          utf8.encode(
            jsonEncode({
              'id': 44,
              'side': 'customer',
              'mine': true,
              'body': body,
              'at': '2026-11-18T09:05:00Z',
              'attachment': null,
            }),
          ),
          201,
        );
      }
      if (path == '/v1/chat/conversations/$id/read') {
        return http.Response('', 204);
      }
      return http.Response('{"error":"chat_not_found"}', 404);
    });
    final api = HttpBackend(cfg(), auth, client: mock);

    expect(await api.chatUnread(), 3);
    final convs = await api.conversations();
    expect(convs.single.id, id);
    final msgs = await api.messages(id, after: 41, limit: 500);
    expect(msgs.map((m) => m.id), [42, 43]);
    expect(
      seen.last,
      'GET https://f-eco.example/v1/chat/conversations/$id/messages?after=41&limit=200',
      reason: 'limit tối đa 200',
    );
    await api.messages(id);
    expect(seen.last, endsWith('/messages?after=0&limit=200'));

    final sent = await api.sendMessage(id, 'Cảm ơn anh');
    expect((sent.id, sent.mine, sent.body), (44, true, 'Cảm ơn anh'));
    expect(
      seen.last,
      'POST https://f-eco.example/v1/chat/conversations/$id/messages',
    );
    expect(bodies.last, {'body': 'Cảm ơn anh'});

    await api.markRead(id, 44);
    expect(
      seen.last,
      'POST https://f-eco.example/v1/chat/conversations/$id/read',
    );
    expect(bodies.last, {'up_to': 44});

    await expectLater(
      api.sendMessage(id, 'bị chặn'),
      throwsA(
        isA<ApiException>()
            .having((e) => e.status, 'status', 409)
            .having((e) => e.code, 'code', 'chat_blocked'),
      ),
    );
    await expectLater(
      api.sendMessage(id, 'nhanh'),
      throwsA(
        isA<ApiException>().having((e) => e.code, 'code', 'chat_rate_limited'),
      ),
    );
    await expectLater(
      api.messages('other'),
      throwsA(
        isA<ApiException>()
            .having((e) => e.status, 'status', 404)
            .having((e) => e.code, 'code', 'chat_not_found'),
      ),
    );
    // Mã hội thoại không an toàn: không gọi máy chủ.
    final before = seen.length;
    await expectLater(
      api.markRead('../wallet', 1),
      throwsA(
        isA<ApiException>().having((e) => e.code, 'code', 'chat_not_found'),
      ),
    );
    await expectLater(api.messages('a/b?x=1'), throwsA(isA<ApiException>()));
    expect(seen.length, before);
  });

  test('chat_thread_send_poll_and_read', () async {
    final backend = DemoBackend();
    final conv = (await backend.conversations()).firstWhere(
      (c) => c.id == DemoBackend.projectChat,
    );
    var reads = 0;
    final thread = ChatThread(
      backend: backend,
      conversation: conv,
      onRead: () async => reads++,
    );
    addTearDown(thread.dispose);
    await thread.load();
    expect(thread.messages.map((m) => m.id), [1, 2, 3, 4, 5]);
    expect((backend.lastRead, reads), ((DemoBackend.projectChat, 5), 1));

    // Kiểm ở máy: không gọi máy chủ.
    final calls = backend.messageCalls;
    expect(await thread.send('   '), 'chat_empty');
    expect(thread.error, 'chat_empty');
    expect(await thread.send('a' * 5001), 'chat_too_long');
    expect(thread.messages.length, 5);

    expect(await thread.send('  Cảm ơn anh '), isNull);
    expect(thread.error, isNull);
    expect(thread.messages.last.body, 'Cảm ơn anh');
    expect(thread.messages.last.mine, isTrue);

    // Tin 5 còn đang quét: hỏi lại từ trước tin 5 để cập nhật trạng thái.
    expect(thread.pollAfter, 4);
    final m = backend.receive(DemoBackend.projectChat, 'Dạ');
    await thread.poll();
    expect(backend.messageCalls, calls + 1);
    expect(
      thread.messages.map((x) => x.id).toSet().length,
      thread.messages.length,
    );
    expect(thread.messages.last.id, m.id);
    expect(backend.lastRead, (DemoBackend.projectChat, m.id));
    expect(reads, 2);
    // Không có tin mới của bên kia: không gọi đánh dấu đã đọc lần nữa.
    await thread.poll();
    expect(reads, 2);
  });

  test('chat_thread_blocked_customer', () async {
    final backend = DemoBackend();
    final conv = (await backend.conversations()).firstWhere(
      (c) => c.id == DemoBackend.blockedChat,
    );
    final thread = ChatThread(backend: backend, conversation: conv);
    addTearDown(thread.dispose);
    await thread.load();
    expect(thread.canSend, isFalse);
    expect(await thread.send('Chào'), 'chat_blocked');
    expect((await backend.messages(DemoBackend.blockedChat)).length, 1);
  });

  // admin-list-tools: mục quản trị chỉ nhận khóa hợp lệ và địa chỉ HTTPS (HTTP chỉ với máy dev).
  test('admin_sections_from_the_server', () {
    final list = AdminSection.listFromJson([
      {'key': 'reviews', 'url': 'https://f-eco.example/admin/reviews'},
      {'key': 'chat-reports', 'url': 'http://127.0.0.1:8080/admin/chat-reports'},
      {'key': 'orders', 'url': 'javascript:alert(1)'},
      {'key': '<b>', 'url': 'https://f-eco.example/admin/x'},
      {'url': 'https://f-eco.example/admin/y'},
    ]);
    expect(list.map((s) => s.key), ['reviews', 'chat-reports']);
    expect(AdminSection.listFromJson(null), isEmpty);
  });
}
