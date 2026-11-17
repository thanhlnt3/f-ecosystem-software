/// Dữ liệu từ API của F-Eco (`/v1/catalog`, `/v1/wallet`, `/v1/library`, `/v1/notifications`).
library;

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
  const Account({required this.name, required this.role});

  final String name;

  /// `user`, `customer`, `business_customer`, `developer`, `dev_company`, `admin`.
  final String role;
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
