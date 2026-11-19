/// Trạng thái của app: đăng nhập, danh mục, ví, thư viện, thông báo, tin nhắn, ngôn ngữ.
library;

import 'package:flutter/foundation.dart';

import 'api.dart';
import 'auth.dart';
import 'chat.dart';
import 'config.dart';
import 'i18n.dart';
import 'models.dart';
import 'web_url.dart';

typedef UrlOpener = Future<bool> Function(Uri url);

class AppState extends ChangeNotifier {
  AppState({
    required this.config,
    required this.auth,
    required this.backend,
    required this.openUrl,
    this._lang = Lang.vi,
  });

  final AppConfig config;
  final Auth auth;
  final Backend backend;
  final UrlOpener openUrl;

  Lang _lang;
  Lang get lang => _lang;
  set lang(Lang l) {
    _lang = l;
    notifyListeners();
  }

  String t(String key, [Map<String, Object> args = const {}]) =>
      tr(_lang, key, args);

  bool get signedIn => auth.signedIn;
  bool busy = false;
  String? error;

  List<CatalogApp> catalog = const [];
  Wallet? wallet;
  List<LibraryItem> library = const [];
  List<NotificationItem> notifications = const [];
  Account? account;

  /// Lúc mở app: khôi phục phiên, tải danh mục và dữ liệu tài khoản.
  Future<void> start() async {
    try {
      await auth.restore();
    } on Exception {
      // Không khôi phục được (mất mạng): vẫn mở app, chưa đăng nhập.
    }
    await reload();
  }

  Future<void> reload() async {
    busy = true;
    error = null;
    notifyListeners();
    try {
      catalog = await backend.catalog();
      if (auth.signedIn) {
        final results = await Future.wait([
          backend.wallet(),
          backend.library(),
          backend.notifications(),
          backend.account(),
        ]);
        wallet = results[0] as Wallet;
        library = results[1] as List<LibraryItem>;
        notifications = results[2] as List<NotificationItem>;
        account = results[3] as Account;
      } else {
        wallet = null;
        library = const [];
        notifications = const [];
        account = null;
      }
    } on Exception {
      error = t('error_network');
    } finally {
      busy = false;
      notifyListeners();
    }
    // Tin nhắn tải riêng: lỗi của chat không che các tab khác.
    if (auth.signedIn) {
      await refreshChat();
    } else {
      chatUnread = 0;
      conversations = const [];
      chatError = null;
      notifyListeners();
    }
  }

  /// Tổng số tin chưa đọc (huy hiệu của tab Tin nhắn).
  int chatUnread = 0;
  List<Conversation> conversations = const [];

  /// Mã lỗi khi tải tin nhắn (hiện bằng `chatErrorMessage`).
  String? chatError;

  /// Số chưa đọc và danh sách hội thoại: khi mở app, kéo làm mới, rời một hội thoại (C3).
  Future<void> refreshChat() async {
    if (!auth.signedIn) return;
    try {
      final results = await Future.wait([
        backend.chatUnread(),
        backend.conversations(),
      ]);
      chatUnread = results[0] as int;
      conversations = results[1] as List<Conversation>;
      chatError = null;
    } on Exception catch (e) {
      chatError = chatErrorCode(e);
    }
    notifyListeners();
  }

  Future<void> signIn() async {
    try {
      await auth.signIn();
      await reload();
    } on Exception {
      error = t('sign_in_failed');
      notifyListeners();
    }
  }

  Future<void> signOut() async {
    await auth.signOut();
    await reload();
  }

  bool inLibrary(String appId) => library.any((i) => i.id == appId);

  /// "Thêm" app web vào Ứng dụng của tôi (ghi lượt thêm lên máy chủ).
  Future<void> add(CatalogApp app) async {
    await backend.addToLibrary(app.id, app.latestVersion);
    library = await backend.library();
    notifyListeners();
  }

  /// Mở app web bằng trình duyệt; địa chỉ không hợp lệ thì không mở.
  Future<bool> openWeb(String? url) async {
    if (!validWebUrl(url)) return false;
    return openUrl(forDevice(Uri.parse(url!), config.server));
  }

  /// Mở một trang của F-Eco trên web (ví, hồ sơ, dự án...).
  Future<bool> openPage(String path) => openUrl(config.page(path));

  int priceOf(CatalogApp app) => wallet?.prices[app.id] ?? app.priceCoin;
  bool owns(String appId) => wallet?.owns(appId) ?? false;

  Future<PurchaseResult> buy(CatalogApp app) async {
    final r = await backend.purchase(app.id, priceOf(app));
    if (r is Purchased) {
      wallet = await backend.wallet();
      library = await backend.library();
      notifyListeners();
    }
    return r;
  }
}
