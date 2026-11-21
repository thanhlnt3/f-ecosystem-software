/// Gọi API của F-Eco (design M4 của thay đổi mobile-launcher).
library;

import 'dart:convert';

import 'package:http/http.dart' as http;

import 'auth.dart';
import 'config.dart';
import 'models.dart';

/// Lỗi từ máy chủ: mã HTTP, mã lỗi (`error`) và thông báo hai thứ tiếng.
class ApiException implements Exception {
  const ApiException(this.status, this.code, this.message, this.messageVi);

  final int status;
  final String code;
  final String message;
  final String messageVi;

  @override
  String toString() => 'ApiException($status $code)';
}

/// Dữ liệu app cần (bản thật gọi máy chủ; bản mẫu trong `demo.dart`).
abstract class Backend {
  Future<List<CatalogApp>> catalog();
  Future<Wallet> wallet();
  Future<PurchaseResult> purchase(String appId, int expectedPrice);
  Future<List<LibraryItem>> library();
  Future<void> addToLibrary(String appId, String version);
  Future<List<NotificationItem>> notifications();
  Future<Account> account();

  /// Logo của app (null: dùng chữ cái đầu).
  Uri? iconUrl(String appId);

  // Tin nhắn (`/v1/chat`, design C4 của thay đổi chat-and-source-scan). Lỗi là [ApiException]
  // với mã của máy chủ (`chat_blocked`, `chat_too_long`...).

  /// Tổng số tin chưa đọc.
  Future<int> chatUnread();

  /// Hội thoại, mới nhất trước.
  Future<List<Conversation>> conversations();

  /// Tin có `id` lớn hơn [after], cũ trước; tối đa [limit] (máy chủ giới hạn 200).
  Future<List<ChatMessage>> messages(
    String conversation, {
    int after = 0,
    int limit = chatPageLimit,
  });

  /// Gửi tin; trả tin đã lưu.
  Future<ChatMessage> sendMessage(String conversation, String body);

  /// Đánh dấu đã đọc tới tin [upTo].
  Future<void> markRead(String conversation, int upTo);
}

/// Số tin tối đa mỗi lần tải (C4).
const chatPageLimit = 200;

/// Mã hội thoại không an toàn để đặt vào đường dẫn: coi như không tìm thấy.
const _badConversation = ApiException(404, 'chat_not_found', '', '');

class HttpBackend implements Backend {
  HttpBackend(this.config, this.auth, {http.Client? client})
    : _http = client ?? http.Client();

  final AppConfig config;
  final Auth auth;
  final http.Client _http;

  Uri _api(String path) => config.server.resolve(path);

  Map<String, Object?> _json(http.Response r) {
    if (r.body.isEmpty) return const {};
    final v = jsonDecode(utf8.decode(r.bodyBytes));
    return v is Map<String, Object?> ? v : const {};
  }

  ApiException _error(http.Response r) {
    final j = _json(r);
    return ApiException(
      r.statusCode,
      j['error'] is String ? j['error'] as String : 'http_${r.statusCode}',
      j['message'] is String ? j['message'] as String : '',
      j['message_vi'] is String ? j['message_vi'] as String : '',
    );
  }

  /// Gọi API cần đăng nhập. Gặp 401 thì làm mới token một lần rồi gọi lại.
  Future<http.Response> _authed(
    String method,
    String path, {
    Object? body,
  }) async {
    Future<http.Response> send(String token) {
      final headers = {
        'Authorization': 'Bearer $token',
        'Accept': 'application/json',
        if (body != null) 'Content-Type': 'application/json',
      };
      return method == 'POST'
          ? _http.post(_api(path), headers: headers, body: jsonEncode(body))
          : _http.get(_api(path), headers: headers);
    }

    var token = await auth.accessToken();
    if (token == null) {
      throw const ApiException(
        401,
        'not_signed_in',
        'Sign in first.',
        'Hãy đăng nhập.',
      );
    }
    var r = await send(token);
    if (r.statusCode == 401) {
      token = await auth.accessToken(refresh: true);
      if (token == null) throw _error(r);
      r = await send(token);
    }
    if (r.statusCode >= 400) throw _error(r);
    return r;
  }

  @override
  Future<List<CatalogApp>> catalog() async {
    final r = await _http.get(
      _api('v1/catalog'),
      headers: {'Accept': 'application/json'},
    );
    if (r.statusCode >= 400) throw _error(r);
    return CatalogApp.listFromJson(_json(r));
  }

  @override
  Future<Wallet> wallet() async =>
      Wallet.fromJson(_json(await _authed('GET', 'v1/wallet')));

  @override
  Future<PurchaseResult> purchase(String appId, int expectedPrice) async {
    try {
      final r = await _authed(
        'POST',
        'v1/wallet/purchases',
        body: {'app_id': appId, 'expected_price': expectedPrice},
      );
      final j = _json(r);
      return Purchased(
        (j['balance'] as num?)?.toInt() ?? 0,
        j['transaction'] is String ? j['transaction'] as String : '',
      );
    } on ApiException catch (e) {
      return PurchaseFailed(
        e.code,
        e.messageVi.isNotEmpty ? e.messageVi : e.message,
      );
    }
  }

  @override
  Future<List<LibraryItem>> library() async =>
      LibraryItem.listFromJson(_json(await _authed('GET', 'v1/library')));

  @override
  Future<void> addToLibrary(String appId, String version) async {
    await _authed(
      'POST',
      'v1/catalog/installs',
      body: {'app_id': appId, 'version': version},
    );
  }

  @override
  Future<List<NotificationItem>> notifications() async =>
      NotificationItem.listFromJson(
        _json(await _authed('GET', 'v1/notifications')),
      );

  @override
  Future<Account> account() async {
    final me = _json(await _authed('GET', 'v1/me'));
    final role = _json(await _authed('GET', 'v1/account/role'));
    return Account(
      name: me['name'] is String ? me['name'] as String : '',
      role: role['role'] is String ? role['role'] as String : 'user',
      adminSections: AdminSection.listFromJson(role['admin_sections']),
    );
  }

  @override
  Uri? iconUrl(String appId) =>
      _api('store/${Uri.encodeComponent(appId)}/icon.png');

  String _conv(String id) {
    if (!safeChatId(id)) throw _badConversation;
    return 'v1/chat/conversations/$id';
  }

  @override
  Future<int> chatUnread() async {
    final v = _json(await _authed('GET', 'v1/chat/unread'))['unread'];
    return v is num && v > 0 ? v.toInt() : 0;
  }

  @override
  Future<List<Conversation>> conversations() async => Conversation.listFromJson(
    _json(await _authed('GET', 'v1/chat/conversations')),
  );

  @override
  Future<List<ChatMessage>> messages(
    String conversation, {
    int after = 0,
    int limit = chatPageLimit,
  }) async {
    final a = after < 0 ? 0 : after;
    final l = limit.clamp(1, chatPageLimit);
    return ChatMessage.listFromJson(
      _json(
        await _authed(
          'GET',
          '${_conv(conversation)}/messages?after=$a&limit=$l',
        ),
      ),
    );
  }

  @override
  Future<ChatMessage> sendMessage(String conversation, String body) async {
    final r = await _authed(
      'POST',
      '${_conv(conversation)}/messages',
      body: {'body': body},
    );
    final m = ChatMessage.tryFromJson(_json(r));
    if (m == null) throw const FormatException('chat: bad message');
    return m;
  }

  @override
  Future<void> markRead(String conversation, int upTo) async {
    await _authed('POST', '${_conv(conversation)}/read', body: {'up_to': upTo});
  }
}
