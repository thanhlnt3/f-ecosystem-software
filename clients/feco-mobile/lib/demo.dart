/// Dữ liệu mẫu (`--dart-define=FECO_DEMO=true`): không gọi mạng, dùng cho integration test trên
/// Android emulator và iOS Simulator, và để xem giao diện.
library;

import 'api.dart';
import 'auth.dart';
import 'models.dart';

class DemoAuth implements Auth {
  bool _in = false;

  @override
  bool get signedIn => _in;

  @override
  Future<bool> restore() async => _in;

  @override
  Future<void> signIn() async => _in = true;

  @override
  Future<void> signOut() async => _in = false;

  @override
  Future<String?> accessToken({bool refresh = false}) async =>
      _in ? 'demo' : null;
}

class DemoBackend implements Backend {
  DemoBackend({this.balance = 10});

  int balance;
  final Set<String> _owned = {};
  final Map<String, String> _library = {};

  static const apps = [
    CatalogApp(
      id: 'thesisai',
      name: 'ThesisAI',
      developer: 'justAchillGuy8386',
      summary: 'Luyện bảo vệ đồ án tốt nghiệp với hội đồng AI.',
      category: 'education',
      priceCoin: 0,
      hasIcon: false,
      webUrl: 'https://thesisai.example/auth/feco',
      platforms: ['web'],
      latestVersion: '0.1.0',
    ),
    CatalogApp(
      id: 'kibi',
      name: 'kibi',
      developer: 'Ilaï Deutel',
      summary: 'Trình soạn thảo văn bản siêu nhỏ chạy trong terminal.',
      category: 'tools',
      priceCoin: 0,
      hasIcon: false,
      webUrl: null,
      platforms: ['linux-x64', 'macos-arm64', 'windows-x64'],
      latestVersion: '0.3.3',
    ),
    CatalogApp(
      id: 'so-tay-pro',
      name: 'Sổ tay Pro',
      developer: 'F-Eco',
      summary: 'Ghi chú có đồng bộ và mẫu trang.',
      category: 'productivity',
      priceCoin: 49,
      hasIcon: false,
      webUrl: 'https://so-tay.example/',
      platforms: ['web'],
      latestVersion: '2.0.0',
    ),
  ];

  @override
  Future<List<CatalogApp>> catalog() async => apps;

  @override
  Future<Wallet> wallet() async => Wallet(
    balance: balance,
    rateVnd: 1000,
    frozen: false,
    owned: _owned,
    prices: {
      for (final a in apps)
        if (a.priceCoin > 0) a.id: a.priceCoin,
    },
    walletUrl: 'https://f-eco.example/wallet',
  );

  @override
  Future<PurchaseResult> purchase(String appId, int expectedPrice) async {
    if (expectedPrice > balance) {
      return PurchaseFailed('insufficient_funds', 'Không đủ F-Coin');
    }
    balance -= expectedPrice;
    _owned.add(appId);
    _library[appId] = apps.firstWhere((a) => a.id == appId).latestVersion;
    return Purchased(balance, 'GD-000001');
  }

  @override
  Future<List<LibraryItem>> library() async => [
    for (final e in _library.entries)
      LibraryItem(
        id: e.key,
        name: apps.firstWhere((a) => a.id == e.key).name,
        hasIcon: false,
        installedVersion: e.value,
        purchased: _owned.contains(e.key),
        latest: apps.firstWhere((a) => a.id == e.key).latestVersion,
        webUrl: apps.firstWhere((a) => a.id == e.key).webUrl,
        updateAvailable: false,
      ),
  ];

  @override
  Future<void> addToLibrary(String appId, String version) async =>
      _library[appId] = version;

  @override
  Future<List<NotificationItem>> notifications() async => [
    NotificationItem(
      id: 1,
      title: 'Chào mừng tới F-Eco',
      body: 'Tài khoản của bạn đã sẵn sàng.',
      link: '/',
      createdAt: DateTime(2026, 11, 17, 9),
      read: false,
    ),
  ];

  @override
  Future<Account> account() async =>
      const Account(name: 'Lan Nguyễn', role: 'customer');

  @override
  Uri? iconUrl(String appId) => null;
}
