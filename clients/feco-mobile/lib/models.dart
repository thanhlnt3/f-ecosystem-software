/// Dữ liệu từ API của F-Eco (`/v1/catalog`, `/v1/wallet`, `/v1/library`, `/v1/notifications`,
/// `/v1/chat`).
library;

import 'web_url.dart';

String _s(Object? v) => v is String ? v : '';
int _i(Object? v) => v is int ? v : (v is num ? v.toInt() : 0);
bool _b(Object? v) => v == true;

/// Một app trong danh mục F-Store.
class CatalogApp {
  const CatalogApp({
    required this.id,
    required this.name,
    required this.developer,
    required this.summary,
    required this.category,
    required this.priceCoin,
    required this.hasIcon,
    required this.webUrl,
    required this.platforms,
    required this.latestVersion,
  });

  final String id;
  final String name;
  final String developer;
  final String summary;
  final String category;
  final int priceCoin;
  final bool hasIcon;

  /// Địa chỉ của app web; null với app desktop.
  final String? webUrl;
  final List<String> platforms;
  final String latestVersion;

  /// App web: mở được trên điện thoại.
  bool get isWeb => webUrl != null && webUrl!.isNotEmpty;

  factory CatalogApp.fromJson(Map<String, Object?> j) => CatalogApp(
    id: _s(j['id']),
    name: _s(j['name']),
    developer: _s(j['developer']),
    summary: _s(j['summary']),
    category: _s(j['category']),
    priceCoin: _i(j['price_coin']),
    hasIcon: _b(j['has_icon']),
    webUrl: j['web_url'] is String && (j['web_url'] as String).isNotEmpty
        ? j['web_url'] as String
        : null,
    platforms: (j['platforms'] is List)
        ? (j['platforms'] as List).whereType<String>().toList()
        : const [],
    latestVersion: _s(j['latest_version']),
  );

  static List<CatalogApp> listFromJson(Map<String, Object?> j) =>
      (j['apps'] is List ? j['apps'] as List : const [])
          .whereType<Map<String, Object?>>()
          .map(CatalogApp.fromJson)
          .toList();
}

/// Ví F-Coin (`GET /v1/wallet`).
class Wallet {
  const Wallet({
    required this.balance,
    required this.rateVnd,
    required this.frozen,
    required this.owned,
    required this.prices,
    required this.walletUrl,
  });

  final int balance;
  final int rateVnd;
  final bool frozen;
  final Set<String> owned;
  final Map<String, int> prices;
  final String walletUrl;

  bool owns(String app) => owned.contains(app);

  factory Wallet.fromJson(Map<String, Object?> j) => Wallet(
    balance: _i(j['balance']),
    rateVnd: _i(j['rate_vnd']),
    frozen: _b(j['frozen']),
    owned: (j['owned'] is List ? j['owned'] as List : const [])
        .whereType<String>()
        .toSet(),
    prices: (j['prices'] is Map ? j['prices'] as Map : const {}).map(
      (k, v) => MapEntry(k.toString(), _i(v)),
    ),
    walletUrl: _s(j['wallet_url']),
  );
}

/// Số F-Coin còn thiếu để mua app giá [price] (0 khi đủ).
int shortfall(int balance, int price) => price > balance ? price - balance : 0;

/// Một app trong "Ứng dụng của tôi" (`GET /v1/library`).
class LibraryItem {
  const LibraryItem({
    required this.id,
    required this.name,
    required this.hasIcon,
    required this.installedVersion,
    required this.purchased,
    required this.latest,
    required this.webUrl,
    required this.updateAvailable,
  });

  final String id;
  final String name;
  final bool hasIcon;
  final String? installedVersion;
  final bool purchased;
  final String? latest;
  final String? webUrl;
  final bool updateAvailable;

  /// App máy tính có bản mới (cập nhật bằng F-Launcher trên máy tính). App web luôn mở bản mới
  /// nhất nên không tính (thay đổi app-update-notices, design U5).
  bool get desktopUpdate => updateAvailable && webUrl == null;

  factory LibraryItem.fromJson(Map<String, Object?> j) => LibraryItem(
    id: _s(j['id']),
    name: _s(j['name']),
    hasIcon: _b(j['has_icon']),
    installedVersion: j['installed_version'] as String?,
    purchased: _b(j['purchased']),
    latest: j['latest'] as String?,
    webUrl: j['web_url'] is String && (j['web_url'] as String).isNotEmpty
        ? j['web_url'] as String
        : null,
    updateAvailable: _b(j['update_available']),
  );

  static List<LibraryItem> listFromJson(Map<String, Object?> j) =>
      (j['apps'] is List ? j['apps'] as List : const [])
          .whereType<Map<String, Object?>>()
          .map(LibraryItem.fromJson)
          .toList();
}

/// Một thông báo (`GET /v1/notifications`).
class NotificationItem {
  const NotificationItem({
    required this.id,
    required this.title,
    required this.body,
    required this.link,
    required this.createdAt,
    required this.read,
  });

  final int id;
  final String title;
  final String body;
  final String link;
  final DateTime? createdAt;
  final bool read;

  factory NotificationItem.fromJson(Map<String, Object?> j) => NotificationItem(
    id: _i(j['id']),
    title: _s(j['title']),
    body: _s(j['body']),
    link: _s(j['link']),
    createdAt: DateTime.tryParse(_s(j['created_at'])),
    read: _b(j['read']),
  );

  static List<NotificationItem> listFromJson(Map<String, Object?> j) =>
      (j['notifications'] is List ? j['notifications'] as List : const [])
          .whereType<Map<String, Object?>>()
          .map(NotificationItem.fromJson)
          .toList()
        ..sort(
          (a, b) => (b.createdAt ?? DateTime(0)).compareTo(
            a.createdAt ?? DateTime(0),
          ),
        );
}

/// Tài khoản: tên (`/v1/me`) và vai trò nền tảng (`/v1/account/role`).
class Account {
  const Account({
    required this.name,
    required this.role,
    this.adminSections = const [],
  });

  final String name;

  /// `user`, `customer`, `business_customer`, `developer`, `dev_company`, `admin`.
  final String role;

  /// Các mục quản trị được vào (thay đổi admin-list-tools, design L6); rỗng với người dùng thường.
  final List<AdminSection> adminSections;
}

/// Một mục của bảng quản trị trên web: khóa (`reviews`, `chat-reports`...) và địa chỉ.
class AdminSection {
  const AdminSection(this.key, this.url);
  final String key;
  final String url;

  static final _key = RegExp(r'^[a-z-]{1,32}$');

  /// `admin_sections` của `GET /v1/account/role`: bỏ mục có khóa lạ hoặc địa chỉ không phải
  /// HTTPS (HTTP chỉ với máy dev).
  static List<AdminSection> listFromJson(Object? j) => [
    for (final e
        in (j is List ? j : const []).whereType<Map<String, Object?>>())
      if (e['key'] is String &&
          _key.hasMatch(e['key'] as String) &&
          e['url'] is String &&
          validWebUrl(e['url'] as String))
        AdminSection(e['key'] as String, e['url'] as String),
  ];
}

/// Kết quả mua app (`POST /v1/wallet/purchases`).
sealed class PurchaseResult {
  const PurchaseResult();
}

class Purchased extends PurchaseResult {
  const Purchased(this.balance, this.transaction);
  final int balance;
  final String transaction;
}

class PurchaseFailed extends PurchaseResult {
  const PurchaseFailed(this.code, this.message);

  /// Mã lỗi của máy chủ, ví dụ `insufficient_funds`, `price_changed`, `wallet_frozen`.
  final String code;
  final String message;
}

// ---------------------------------------------------------------------------------------------
// Tin nhắn (`/v1/chat`, design C4 của thay đổi chat-and-source-scan).

/// Mã hội thoại dùng trong đường dẫn API: chỉ chữ, số, `-`, `_` (uuid của máy chủ).
final _safeId = RegExp(r'^[A-Za-z0-9_-]{1,64}$');

bool safeChatId(String id) => _safeId.hasMatch(id);

/// Độ dài tối đa của một tin (C2), tính theo ký tự sau khi bỏ khoảng trắng hai đầu.
const chatMaxChars = 5000;

/// Kiểm tin trước khi gửi (máy chủ vẫn kiểm lại): trả mã lỗi `chat_empty`, `chat_too_long`, hoặc
/// null khi gửi được.
String? validateChatBody(String body) {
  final t = body.trim();
  if (t.isEmpty) return 'chat_empty';
  if (t.runes.length > chatMaxChars) return 'chat_too_long';
  return null;
}

String? _url(Object? v) => v is String && validWebUrl(v) ? v.trim() : null;
String? _opt(Object? v) => v is String && v.isNotEmpty ? v : null;

enum ConversationKind { direct, project }

enum ChatRole { customer, developer }

/// Một hội thoại (`GET /v1/chat/conversations`).
class Conversation {
  const Conversation({
    required this.id,
    required this.kind,
    required this.role,
    required this.peerName,
    required this.title,
    this.projectId,
    this.appId,
    this.blocked = false,
    this.lastMessage,
    this.lastMessageAt,
    this.unread = 0,
    this.webUrl,
  });

  final String id;
  final ConversationKind kind;

  /// Vai trò của mình trong hội thoại.
  final ChatRole role;
  final String peerName;
  final String title;
  final String? projectId;
  final String? appId;
  final bool blocked;
  final String? lastMessage;
  final DateTime? lastMessageAt;
  final int unread;

  /// Trang hội thoại trên web (gửi tệp đính kèm); null khi địa chỉ không hợp lệ.
  final String? webUrl;

  /// Khách hàng trong hội thoại `direct` bị nhà phát triển chặn: không gửi được.
  bool get customerBlocked =>
      blocked && kind == ConversationKind.direct && role == ChatRole.customer;

  Conversation copyWith({bool? blocked}) => Conversation(
    id: id,
    kind: kind,
    role: role,
    peerName: peerName,
    title: title,
    projectId: projectId,
    appId: appId,
    blocked: blocked ?? this.blocked,
    lastMessage: lastMessage,
    lastMessageAt: lastMessageAt,
    unread: unread,
    webUrl: webUrl,
  );

  /// Null khi mục hỏng (mã không an toàn, `kind` hoặc `role` lạ).
  static Conversation? tryFromJson(Object? raw) {
    if (raw is! Map<String, Object?>) return null;
    final id = raw['id'];
    if (id is! String || !safeChatId(id)) return null;
    final kind = ConversationKind.values.asNameMap()[raw['kind']];
    final role = ChatRole.values.asNameMap()[raw['role']];
    if (kind == null || role == null) return null;
    final unread = _i(raw['unread']);
    return Conversation(
      id: id,
      kind: kind,
      role: role,
      peerName: _s(raw['peer_name']),
      title: _s(raw['title']),
      projectId: _opt(raw['project_id']),
      appId: _opt(raw['app_id']),
      blocked: _b(raw['blocked']),
      lastMessage: _opt(raw['last_message']),
      lastMessageAt: DateTime.tryParse(_s(raw['last_message_at'])),
      unread: unread < 0 ? 0 : unread,
      webUrl: _url(raw['web_url']),
    );
  }

  /// `{"conversations": [...]}`, giữ thứ tự của máy chủ (mới nhất trước), bỏ mục hỏng.
  static List<Conversation> listFromJson(Map<String, Object?> j) =>
      (j['conversations'] is List ? j['conversations'] as List : const [])
          .map(tryFromJson)
          .whereType<Conversation>()
          .toList();
}

enum MessageSide { customer, developer, admin, system }

enum AttachmentStatus { pending, clean, infected, error }

/// Tệp đính kèm (chỉ xem trên điện thoại; gửi tệp qua web).
class ChatAttachment {
  const ChatAttachment({
    required this.name,
    required this.size,
    required this.status,
    this.url,
  });

  final String name;
  final int size;

  /// Trạng thái lạ thì coi như `error` (không mở).
  final AttachmentStatus status;
  final String? url;

  /// Chỉ tệp đã quét sạch và có địa chỉ hợp lệ mới mở được.
  bool get canOpen => status == AttachmentStatus.clean && url != null;

  static ChatAttachment? tryFromJson(Object? raw) {
    if (raw is! Map<String, Object?>) return null;
    final size = _i(raw['size']);
    return ChatAttachment(
      name: _s(raw['name']).isEmpty ? '?' : _s(raw['name']),
      size: size < 0 ? 0 : size,
      status:
          AttachmentStatus.values.asNameMap()[raw['status']] ??
          AttachmentStatus.error,
      url: _url(raw['url']),
    );
  }
}

/// Một tin (`GET /v1/chat/conversations/{id}/messages`).
class ChatMessage {
  const ChatMessage({
    required this.id,
    required this.side,
    required this.mine,
    required this.body,
    this.at,
    this.attachment,
  });

  final int id;
  final MessageSide side;
  final bool mine;
  final String body;
  final DateTime? at;
  final ChatAttachment? attachment;

  /// Null khi mục hỏng (không có `id` dương, `side` lạ).
  static ChatMessage? tryFromJson(Object? raw) {
    if (raw is! Map<String, Object?>) return null;
    final id = raw['id'];
    if (id is! int || id <= 0) return null;
    final side = MessageSide.values.asNameMap()[raw['side']];
    if (side == null) return null;
    return ChatMessage(
      id: id,
      side: side,
      mine: _b(raw['mine']),
      body: _s(raw['body']),
      at: DateTime.tryParse(_s(raw['at'])),
      attachment: ChatAttachment.tryFromJson(raw['attachment']),
    );
  }

  /// `{"messages": [...]}`, bỏ mục hỏng, sắp theo `id` tăng dần, không trùng.
  static List<ChatMessage> listFromJson(Map<String, Object?> j) =>
      mergeMessages(
        const [],
        (j['messages'] is List ? j['messages'] as List : const [])
            .map(tryFromJson)
            .whereType<ChatMessage>(),
      );
}

/// Gộp tin mới vào luồng: theo `id`, không trùng (tin mới thay tin cũ cùng `id`, ví dụ khi
/// trạng thái quét đính kèm đổi), sắp tăng dần.
List<ChatMessage> mergeMessages(
  Iterable<ChatMessage> current,
  Iterable<ChatMessage> incoming,
) {
  final byId = <int, ChatMessage>{
    for (final m in current) m.id: m,
    for (final m in incoming) m.id: m,
  };
  return byId.values.toList()..sort((a, b) => a.id.compareTo(b.id));
}
