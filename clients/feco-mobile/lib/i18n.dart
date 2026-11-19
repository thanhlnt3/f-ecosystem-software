/// Chuỗi giao diện bốn thứ tiếng (spec mobile-launcher, "Ngôn ngữ trên điện thoại"). Mỗi khóa có
/// đủ [vi, en, fr, ja]; test `every_key_has_four_languages` kiểm điều này.
library;

enum Lang {
  vi('vi', 'Tiếng Việt'),
  en('en', 'English'),
  fr('fr', 'Français'),
  ja('ja', '日本語');

  const Lang(this.code, this.label);
  final String code;
  final String label;

  static Lang fromCode(String? code) =>
      Lang.values.firstWhere((l) => l.code == code, orElse: () => Lang.vi);
}

const Map<String, List<String>> strings = {
  'tab.library': ['Ứng dụng của tôi', 'My apps', 'Mes applis', 'マイアプリ'],
  'tab.store': ['Cửa hàng', 'Store', 'Boutique', 'ストア'],
  'tab.wallet': ['Ví', 'Wallet', 'Portefeuille', 'ウォレット'],
  'tab.notifications': ['Thông báo', 'Notifications', 'Notifications', 'お知らせ'],
  'tab.account': ['Tài khoản', 'Account', 'Compte', 'アカウント'],
  'tab.messages': ['Tin nhắn', 'Messages', 'Messages', 'メッセージ'],
  'chat.no_conversations': [
    'Chưa có hội thoại nào. Nhắn cho nhà phát triển từ trang app hoặc trang dự án trên web.',
    'No conversations yet. Message a developer from an app or project page on the web.',
    'Aucune conversation. Écrivez à un développeur depuis la page d’une appli ou d’un projet en ligne.',
    '会話はまだありません。ウェブのアプリまたはプロジェクトのページから開発者にメッセージを送れます。',
  ],
  'chat.no_messages': [
    'Chưa có tin nhắn.',
    'No messages yet.',
    'Aucun message.',
    'メッセージはまだありません。',
  ],
  'chat.project': ['Dự án', 'Project', 'Projet', 'プロジェクト'],
  'chat.unread': [
    '{n} tin chưa đọc',
    '{n} unread',
    '{n} non lu(s)',
    '未読 {n} 件',
  ],
  'chat.input_hint': [
    'Nhập tin nhắn',
    'Write a message',
    'Écrire un message',
    'メッセージを入力',
  ],
  'chat.send': ['Gửi', 'Send', 'Envoyer', '送信'],
  'chat.open_web': [
    'Mở trên web',
    'Open on the web',
    'Ouvrir en ligne',
    'ウェブで開く',
  ],
  'chat.attach_on_web': [
    'Gửi tệp đính kèm trên web',
    'Send attachments on the web',
    'Envoyer des pièces jointes en ligne',
    '添付ファイルはウェブで送信',
  ],
  'chat.blocked_customer': [
    'Nhà phát triển đã chặn hội thoại này nên bạn không gửi được tin nhắn.',
    'The developer has blocked this conversation, so you can’t send messages.',
    'Le développeur a bloqué cette conversation : vous ne pouvez pas envoyer de messages.',
    '開発者がこの会話をブロックしたため、メッセージを送信できません。',
  ],
  'chat.blocked_developer': [
    'Bạn đã chặn hội thoại này. Bỏ chặn trên web.',
    'You have blocked this conversation. Unblock it on the web.',
    'Vous avez bloqué cette conversation. Débloquez-la en ligne.',
    'この会話はブロック中です。ブロック解除はウェブで行えます。',
  ],
  'chat.side.admin': [
    'Quản trị F-Eco',
    'F-Eco admin',
    'Admin F-Eco',
    'F-Eco 管理者',
  ],
  'chat.att.pending': [
    'Đang kiểm tra',
    'Being checked',
    'Analyse en cours',
    '確認中',
  ],
  'chat.att.clean': [
    'Đã kiểm tra, an toàn',
    'Checked, safe',
    'Analysée, sûre',
    '確認済み・安全',
  ],
  'chat.att.infected': [
    'Có mã độc, đã chặn',
    'Malware found, blocked',
    'Logiciel malveillant détecté, bloquée',
    'マルウェアを検出、ブロック済み',
  ],
  'chat.att.error': [
    'Không kiểm tra được, không mở được',
    'Couldn’t be checked, can’t be opened',
    'Analyse impossible, ouverture bloquée',
    '確認できなかったため開けません',
  ],
  'chat.size_b': ['{n} B', '{n} B', '{n} o', '{n} B'],
  'chat.size_kb': ['{n} KB', '{n} KB', '{n} Ko', '{n} KB'],
  'chat.size_mb': ['{n} MB', '{n} MB', '{n} Mo', '{n} MB'],
  'chat.err.chat_blocked': [
    'Bạn không gửi được tin vì nhà phát triển đã chặn hội thoại này.',
    'You can’t send messages because the developer has blocked this conversation.',
    'Vous ne pouvez pas envoyer de messages : le développeur a bloqué cette conversation.',
    '開発者がこの会話をブロックしたため、送信できません。',
  ],
  'chat.err.chat_too_long': [
    'Tin nhắn dài quá 5.000 ký tự. Hãy rút gọn hoặc chia thành nhiều tin.',
    'The message is longer than 5,000 characters. Shorten it or split it up.',
    'Le message dépasse 5 000 caractères. Raccourcissez-le ou découpez-le.',
    'メッセージが 5,000 文字を超えています。短くするか分けて送ってください。',
  ],
  'chat.err.chat_empty': [
    'Hãy nhập nội dung tin nhắn.',
    'Type a message first.',
    'Saisissez d’abord un message.',
    'メッセージを入力してください。',
  ],
  'chat.err.chat_rate_limited': [
    'Bạn gửi quá nhanh. Hãy đợi một lát rồi gửi lại.',
    'You’re sending too fast. Wait a moment and try again.',
    'Vous envoyez trop vite. Patientez un instant puis réessayez.',
    '送信が速すぎます。少し待ってからもう一度送ってください。',
  ],
  'chat.err.chat_forbidden': [
    'Bạn không tham gia hội thoại này.',
    'You’re not part of this conversation.',
    'Vous ne participez pas à cette conversation.',
    'この会話の参加者ではありません。',
  ],
  'chat.err.chat_not_found': [
    'Không tìm thấy hội thoại.',
    'Conversation not found.',
    'Conversation introuvable.',
    '会話が見つかりません。',
  ],
  'chat.err.chat_not_developer': [
    'Người này không phải nhà phát triển.',
    'This person isn’t a developer.',
    'Cette personne n’est pas développeur.',
    'この相手は開発者ではありません。',
  ],
  'chat.err.chat_self': [
    'Bạn không thể nhắn tin cho chính mình.',
    'You can’t message yourself.',
    'Vous ne pouvez pas vous écrire à vous-même.',
    '自分自身にはメッセージを送れません。',
  ],
  'chat.err.generic': [
    'Có lỗi với tin nhắn. Hãy thử lại.',
    'Something went wrong with messages. Try again.',
    'Un problème est survenu avec les messages. Réessayez.',
    'メッセージでエラーが発生しました。もう一度お試しください。',
  ],
  'sign_in': ['Đăng nhập', 'Sign in', 'Se connecter', 'サインイン'],
  'sign_out': ['Đăng xuất', 'Sign out', 'Se déconnecter', 'サインアウト'],
  'need_sign_in': [
    'Đăng nhập bằng tài khoản F-Eco để dùng mục này.',
    'Sign in with your F-Eco account to use this.',
    'Connectez-vous avec votre compte F-Eco pour utiliser cette section.',
    'このページを使うには F-Eco アカウントでサインインしてください。',
  ],
  'sign_in_failed': [
    'Đăng nhập không thành công.',
    'Sign-in failed.',
    'La connexion a échoué.',
    'サインインできませんでした。',
  ],
  'search': [
    'Tìm theo tên, mô tả, nhà phát triển',
    'Search by name, description, developer',
    'Rechercher par nom, description, développeur',
    '名前・説明・開発者で検索',
  ],
  'all_categories': [
    'Mọi nhóm',
    'All categories',
    'Toutes les catégories',
    'すべてのカテゴリ',
  ],
  'open': ['Mở', 'Open', 'Ouvrir', '開く'],
  'add': ['Thêm', 'Add', 'Ajouter', '追加'],
  'added': ['Đã thêm', 'Added', 'Ajoutée', '追加済み'],
  'web_app': ['App web', 'Web app', 'App web', 'Web アプリ'],
  'desktop_only': [
    'Dùng trên máy tính qua F-Launcher',
    'Use it on a computer with F-Launcher',
    'À utiliser sur ordinateur avec F-Launcher',
    'パソコンの F-Launcher で利用',
  ],
  'free': ['Miễn phí', 'Free', 'Gratuit', '無料'],
  'buy': [
    'Mua · {n} F-Coin',
    'Buy · {n} F-Coin',
    'Acheter · {n} F-Coin',
    '購入 · {n} F-Coin',
  ],
  'coins': ['{n} F-Coin', '{n} F-Coin', '{n} F-Coin', '{n} F-Coin'],
  'balance': ['Số dư', 'Balance', 'Solde', '残高'],
  'rate': [
    '1 F-Coin = {vnd} ₫',
    '1 F-Coin = {vnd} VND',
    '1 F-Coin = {vnd} VND',
    '1 F-Coin = {vnd} ドン',
  ],
  'topup': [
    'Nạp F-Coin',
    'Top up F-Coin',
    'Recharger des F-Coin',
    'F-Coin をチャージ',
  ],
  'withdraw': [
    'Rút F-Coin',
    'Withdraw F-Coin',
    'Retirer des F-Coin',
    'F-Coin を出金',
  ],
  'wallet_frozen': [
    'Ví F-Coin đang bị khóa',
    'Your F-Coin wallet is frozen',
    'Votre portefeuille F-Coin est bloqué',
    'F-Coin ウォレットは凍結されています',
  ],
  'wallet_web_hint': [
    'Nạp, rút và xem lịch sử giao dịch trên F-Eco trên web.',
    'Top up, withdraw and see your history on F-Eco on the web.',
    'Rechargez, retirez et consultez l’historique sur F-Eco en ligne.',
    'チャージ・出金・履歴はウェブ版 F-Eco で行えます。',
  ],
  'confirm_buy_title': [
    'Mua {app}?',
    'Buy {app}?',
    'Acheter {app} ?',
    '{app} を購入しますか?',
  ],
  'confirm_buy_body': [
    'Giá {price} F-Coin. Số dư sau khi mua: {after} F-Coin.',
    'Price {price} F-Coin. Balance after buying: {after} F-Coin.',
    'Prix {price} F-Coin. Solde après l’achat : {after} F-Coin.',
    '価格 {price} F-Coin。購入後の残高: {after} F-Coin。',
  ],
  'short': [
    'Còn thiếu {n} F-Coin',
    '{n} F-Coin short',
    'Il manque {n} F-Coin',
    '{n} F-Coin 不足しています',
  ],
  'cancel': ['Hủy', 'Cancel', 'Annuler', 'キャンセル'],
  'bought': ['Đã mua {app}', 'Bought {app}', '{app} achetée', '{app} を購入しました'],
  'update_available': [
    'Có bản mới',
    'Update available',
    'Mise à jour disponible',
    '新しいバージョンあり',
  ],
  'purchased': ['Đã mua', 'Purchased', 'Achetée', '購入済み'],
  'library_empty': [
    'Chưa có app nào. Thêm app web hoặc mua app trong Cửa hàng.',
    'No apps yet. Add a web app or buy an app in the Store.',
    'Aucune appli pour l’instant. Ajoutez une app web ou achetez une appli dans la Boutique.',
    'まだアプリがありません。ストアで Web アプリを追加するか、アプリを購入してください。',
  ],
  'no_notifications': [
    'Chưa có thông báo.',
    'No notifications yet.',
    'Aucune notification.',
    'お知らせはまだありません。',
  ],
  'role': ['Vai trò', 'Role', 'Rôle', 'ロール'],
  'language': ['Ngôn ngữ', 'Language', 'Langue', '言語'],
  'on_web': [
    'F-Eco trên web',
    'F-Eco on the web',
    'F-Eco en ligne',
    'ウェブ版 F-Eco',
  ],
  'profile_web': [
    'Hồ sơ và vai trò',
    'Profile and role',
    'Profil et rôle',
    'プロフィールとロール',
  ],
  'projects_web': [
    'Dự án phần mềm',
    'Software projects',
    'Projets logiciels',
    'ソフトウェアプロジェクト',
  ],
  'orders_web': [
    'Đơn xây app',
    'Custom app orders',
    'Commandes d’applis',
    'アプリ開発の注文',
  ],
  'error_network': [
    'Không kết nối được máy chủ F-Eco.',
    'Can’t reach the F-Eco server.',
    'Impossible de joindre le serveur F-Eco.',
    'F-Eco サーバーに接続できません。',
  ],
  'retry': ['Thử lại', 'Retry', 'Réessayer', '再試行'],
  'version': ['Phiên bản {v}', 'Version {v}', 'Version {v}', 'バージョン {v}'],
  'demo': [
    'Chế độ dữ liệu mẫu',
    'Sample data mode',
    'Mode données d’exemple',
    'サンプルデータモード',
  ],
  'role.user': [
    'Người dùng phổ thông',
    'General user',
    'Utilisateur standard',
    '一般ユーザー',
  ],
  'role.customer': [
    'Khách hàng cá nhân',
    'Individual customer',
    'Client particulier',
    '個人顧客',
  ],
  'role.business_customer': [
    'Khách hàng doanh nghiệp',
    'Business customer',
    'Client entreprise',
    '法人顧客',
  ],
  'role.developer': [
    'Lập trình viên cá nhân',
    'Individual developer',
    'Développeur indépendant',
    '個人開発者',
  ],
  'role.dev_company': [
    'Công ty phát triển phần mềm',
    'Software company',
    'Société de développement logiciel',
    'ソフトウェア開発会社',
  ],
  'role.admin': ['Quản trị viên', 'Administrator', 'Administrateur', '管理者'],
  'cat.productivity': ['Năng suất', 'Productivity', 'Productivité', '仕事効率化'],
  'cat.tools': ['Công cụ', 'Tools', 'Outils', 'ツール'],
  'cat.system': ['Hệ thống', 'System', 'Système', 'システム'],
  'cat.development': ['Lập trình', 'Development', 'Développement', '開発'],
  'cat.education': ['Giáo dục', 'Education', 'Éducation', '教育'],
  'cat.other': ['Khác', 'Other', 'Autre', 'その他'],
};

/// Chuỗi của [key] theo [lang], thay `{tên}` bằng giá trị trong [args]. Khóa không có thì trả
/// về chính khóa (test bảo đảm không xảy ra).
String tr(Lang lang, String key, [Map<String, Object> args = const {}]) {
  var text = strings[key]?[lang.index] ?? key;
  args.forEach((k, v) => text = text.replaceAll('{$k}', '$v'));
  return text;
}

/// Tên nhóm app; nhóm lạ thì viết hoa chữ đầu.
String categoryLabel(Lang lang, String code) {
  final key = 'cat.$code';
  if (strings.containsKey(key)) return tr(lang, key);
  if (code.isEmpty) return tr(lang, 'cat.other');
  return code[0].toUpperCase() + code.substring(1);
}

/// Số có dấu phân cách hàng nghìn (1.000 kiểu Việt, 1,000 kiểu Anh).
String group(int n, Lang lang) {
  final sep = lang == Lang.vi || lang == Lang.fr ? '.' : ',';
  final digits = n.abs().toString();
  final out = StringBuffer();
  for (var i = 0; i < digits.length; i++) {
    if (i > 0 && (digits.length - i) % 3 == 0) out.write(sep);
    out.write(digits[i]);
  }
  return n < 0 ? '-$out' : out.toString();
}
