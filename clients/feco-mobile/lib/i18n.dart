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
