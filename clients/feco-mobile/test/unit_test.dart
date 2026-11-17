// Unit test của F-Launcher di động (thay đổi mobile-launcher, design M5).

import 'dart:convert';

import 'package:feco_mobile/api.dart';
import 'package:feco_mobile/auth.dart';
import 'package:feco_mobile/config.dart';
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
}
