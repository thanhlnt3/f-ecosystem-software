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

  /// Vai trò và mục quản trị của tài khoản mẫu (widget test bảng quản trị).
  String role = 'customer';
  List<AdminSection> adminSections = const [];
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
        updateAvailable:
            e.value != apps.firstWhere((a) => a.id == e.key).latestVersion,
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
      Account(name: 'Lan Nguyễn', role: role, adminSections: adminSections);

  @override
  Uri? iconUrl(String appId) => null;

  // Tin nhắn mẫu.

  static const projectChat = 'c0ffee00-0000-4000-8000-000000000001';
  static const appChat = 'c0ffee00-0000-4000-8000-000000000002';
  static const blockedChat = 'c0ffee00-0000-4000-8000-000000000003';

  static String _web(String id) => 'https://f-eco.example/messages/$id';

  final Map<String, Conversation> _convs = {
    projectChat: Conversation(
      id: projectChat,
      kind: ConversationKind.project,
      role: ChatRole.customer,
      peerName: 'Minh Trần',
      title: 'Website bán hàng',
      projectId: 'b5a1c3d2-0000-4000-8000-000000000010',
      webUrl: _web(projectChat),
    ),
    appChat: Conversation(
      id: appChat,
      kind: ConversationKind.direct,
      role: ChatRole.customer,
      peerName: 'justAchillGuy8386',
      title: 'ThesisAI',
      appId: 'thesisai',
      webUrl: _web(appChat),
    ),
    blockedChat: Conversation(
      id: blockedChat,
      kind: ConversationKind.direct,
      role: ChatRole.customer,
      peerName: 'Hoa Lê',
      title: 'Hoa Lê',
      blocked: true,
      webUrl: _web(blockedChat),
    ),
  };

  final Map<String, List<ChatMessage>> _msgs = {
    projectChat: [
      ChatMessage(
        id: 1,
        side: MessageSide.system,
        mine: false,
        body: 'Dự án đã bắt đầu.',
        at: DateTime.utc(2026, 11, 16, 2),
      ),
      ChatMessage(
        id: 2,
        side: MessageSide.developer,
        mine: false,
        body: 'Chào chị, em gửi bản thiết kế trang chủ.',
        at: DateTime.utc(2026, 11, 16, 2, 5),
        attachment: ChatAttachment(
          name: 'thiet-ke-trang-chu.pdf',
          size: 482133,
          status: AttachmentStatus.clean,
          url: 'https://f-eco.example/messages/$projectChat/files/a1',
        ),
      ),
      ChatMessage(
        id: 3,
        side: MessageSide.customer,
        mine: true,
        body: 'Đẹp lắm. Thêm mục khuyến mãi được không anh?',
        at: DateTime.utc(2026, 11, 16, 3),
      ),
      ChatMessage(
        id: 4,
        side: MessageSide.admin,
        mine: false,
        body: 'Quản trị viên đã xem tiến độ: đúng hạn.',
        at: DateTime.utc(2026, 11, 17, 1),
      ),
      ChatMessage(
        id: 5,
        side: MessageSide.developer,
        mine: false,
        body: 'Được chị, em làm xong trong tuần này.',
        at: DateTime.utc(2026, 11, 17, 2),
        attachment: ChatAttachment(
          name: 'khuyen-mai.png',
          size: 120400,
          status: AttachmentStatus.pending,
        ),
      ),
    ],
    appChat: [
      ChatMessage(
        id: 6,
        side: MessageSide.customer,
        mine: true,
        body: 'ThesisAI có hỗ trợ tiếng Anh không?',
        at: DateTime.utc(2026, 11, 15, 8),
      ),
      ChatMessage(
        id: 7,
        side: MessageSide.developer,
        mine: false,
        body: 'Có bạn nhé, chọn ngôn ngữ trong phần cài đặt.',
        at: DateTime.utc(2026, 11, 15, 9),
        attachment: ChatAttachment(
          name: 'huong-dan.exe',
          size: 2048,
          status: AttachmentStatus.infected,
        ),
      ),
    ],
    blockedChat: [
      ChatMessage(
        id: 8,
        side: MessageSide.customer,
        mine: true,
        body: 'Chào chị.',
        at: DateTime.utc(2026, 11, 10, 8),
      ),
    ],
  };

  /// Đã đọc tới tin nào trong mỗi hội thoại.
  final Map<String, int> _read = {projectChat: 3, appChat: 7, blockedChat: 8};

  /// Số lần gọi `messages` (test kiểm việc hỏi tin mới).
  int messageCalls = 0;

  /// Lần đánh dấu đã đọc gần nhất (hội thoại, tới tin).
  (String, int)? lastRead;

  int get _nextId =>
      _msgs.values.expand((l) => l).fold(0, (a, m) => m.id > a ? m.id : a) + 1;

  int _unreadOf(String id) =>
      _msgs[id]!.where((m) => !m.mine && m.id > (_read[id] ?? 0)).length;

  Conversation _withSummary(Conversation c) {
    final last = _msgs[c.id]!.last;
    return Conversation(
      id: c.id,
      kind: c.kind,
      role: c.role,
      peerName: c.peerName,
      title: c.title,
      projectId: c.projectId,
      appId: c.appId,
      blocked: c.blocked,
      lastMessage: last.body.length > 120
          ? last.body.substring(0, 120)
          : last.body,
      lastMessageAt: last.at,
      unread: _unreadOf(c.id),
      webUrl: c.webUrl,
    );
  }

  /// Bên kia gửi một tin (test, xem giao diện).
  ChatMessage receive(String conversation, String body) {
    final m = ChatMessage(
      id: _nextId,
      side: MessageSide.developer,
      mine: false,
      body: body,
      at: DateTime.now().toUtc(),
    );
    _msgs[conversation]!.add(m);
    return m;
  }

  static const _notFound = ApiException(404, 'chat_not_found', '', '');

  @override
  Future<int> chatUnread() async =>
      _convs.keys.fold<int>(0, (a, id) => a + _unreadOf(id));

  @override
  Future<List<Conversation>> conversations() async {
    final list = _convs.values.map(_withSummary).toList()
      ..sort(
        (a, b) => (b.lastMessageAt ?? DateTime(0)).compareTo(
          a.lastMessageAt ?? DateTime(0),
        ),
      );
    return list;
  }

  @override
  Future<List<ChatMessage>> messages(
    String conversation, {
    int after = 0,
    int limit = chatPageLimit,
  }) async {
    messageCalls++;
    final list = _msgs[conversation];
    if (list == null) throw _notFound;
    return list.where((m) => m.id > after).take(limit).toList();
  }

  @override
  Future<ChatMessage> sendMessage(String conversation, String body) async {
    final c = _convs[conversation];
    if (c == null) throw _notFound;
    if (c.customerBlocked) {
      throw const ApiException(409, 'chat_blocked', '', '');
    }
    final invalid = validateChatBody(body);
    if (invalid != null) throw ApiException(422, invalid, '', '');
    final m = ChatMessage(
      id: _nextId,
      side: c.role == ChatRole.customer
          ? MessageSide.customer
          : MessageSide.developer,
      mine: true,
      body: body.trim(),
      at: DateTime.now().toUtc(),
    );
    _msgs[conversation]!.add(m);
    return m;
  }

  @override
  Future<void> markRead(String conversation, int upTo) async {
    if (!_convs.containsKey(conversation)) throw _notFound;
    lastRead = (conversation, upTo);
    if (upTo > (_read[conversation] ?? 0)) _read[conversation] = upTo;
  }
}
