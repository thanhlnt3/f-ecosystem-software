//! Ngôn ngữ giao diện của launcher: tiếng Việt, tiếng Anh, tiếng Pháp (spec launcher của thay
//! đổi launcher-personalization; design P1).
//!
//! Khóa là chuỗi tiếng Việt gốc: `t("Thư viện")` trả bản dịch theo ngôn ngữ đang chọn, chuỗi
//! chưa có trong bảng thì giữ tiếng Việt. Chuỗi có tham số dùng `tf("Đã cài {} {}", &[&a, &b])`;
//! bản dịch giữ đúng số `{}`. Test trong mô-đun này kiểm mọi chuỗi trong mã giao diện đều có bản
//! tiếng Anh và tiếng Pháp.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt::Display;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::OnceLock;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Language {
    #[default]
    Vi,
    En,
    Fr,
}

impl Language {
    pub const ALL: [Language; 3] = [Language::Vi, Language::En, Language::Fr];

    /// Tên ngôn ngữ viết bằng chính ngôn ngữ đó.
    pub const fn native_name(self) -> &'static str {
        match self {
            Language::Vi => "Tiếng Việt",
            Language::En => "English",
            Language::Fr => "Français",
        }
    }
}

static CURRENT: AtomicU8 = AtomicU8::new(0);

pub fn set_language(lang: Language) {
    CURRENT.store(lang as u8, Ordering::Relaxed);
}

pub fn language() -> Language {
    match CURRENT.load(Ordering::Relaxed) {
        1 => Language::En,
        2 => Language::Fr,
        _ => Language::Vi,
    }
}

fn table() -> &'static HashMap<&'static str, (&'static str, &'static str)> {
    static MAP: OnceLock<HashMap<&'static str, (&'static str, &'static str)>> = OnceLock::new();
    MAP.get_or_init(|| TABLE.iter().map(|(vi, en, fr)| (*vi, (*en, *fr))).collect())
}

/// Bản dịch của `vi` theo ngôn ngữ đang chọn.
pub fn t(vi: &'static str) -> &'static str {
    translate(language(), vi)
}

pub fn translate(lang: Language, vi: &'static str) -> &'static str {
    match (lang, table().get(vi)) {
        (Language::En, Some((en, _))) => en,
        (Language::Fr, Some((_, fr))) => fr,
        _ => vi,
    }
}

/// Như [`t`], rồi điền lần lượt `args` vào các chỗ `{}`.
pub fn tf(vi: &'static str, args: &[&dyn Display]) -> String {
    fill(t(vi), args)
}

fn fill(template: &str, args: &[&dyn Display]) -> String {
    let mut out = String::with_capacity(template.len() + 16);
    let mut rest = template;
    let mut args = args.iter();
    while let Some(i) = rest.find("{}") {
        out.push_str(&rest[..i]);
        if let Some(a) = args.next() {
            out.push_str(&a.to_string());
        }
        rest = &rest[i + 2..];
    }
    out.push_str(rest);
    out
}

/// (tiếng Việt, tiếng Anh, tiếng Pháp).
const TABLE: &[(&str, &str, &str)] = &[
    // Thanh bên, trang
    ("Thư viện", "Library", "Bibliothèque"),
    ("Cửa hàng", "Store", "Boutique"),
    ("Thông báo", "Notifications", "Notifications"),
    ("Cài đặt", "Settings", "Paramètres"),
    ("Thư viện  ({} cập nhật)", "Library  ({} updates)", "Bibliothèque  ({} mises à jour)"),
    ("Thông báo  ({})", "Notifications  ({})", "Notifications  ({})"),
    ("Đã kết nối", "Connected", "Connecté"),
    ("Đang ngoại tuyến", "Offline", "Hors ligne"),
    ("Đồng bộ", "Sync", "Synchroniser"),
    ("Đồng bộ lúc {}", "Synced at {}", "Synchronisé à {}"),
    ("Đăng nhập", "Sign in", "Se connecter"),
    ("Đăng xuất", "Sign out", "Se déconnecter"),
    ("Đã đăng nhập", "Signed in", "Connecté"),
    ("Chưa đăng nhập", "Not signed in", "Non connecté"),
    ("Chưa đăng nhập.", "Not signed in.", "Non connecté."),
    // Thanh công cụ
    ("{} app", "{} apps", "{} applis"),
    ("Tìm theo tên, mô tả, nhà phát triển...", "Search by name, description, developer...", "Rechercher par nom, description, développeur..."),
    ("Mọi nhóm", "All categories", "Toutes les catégories"),
    ("Nhóm: {}", "Category: {}", "Catégorie : {}"),
    ("Sắp xếp: {}", "Sort: {}", "Tri : {}"),
    ("Xóa bộ lọc", "Clear filters", "Effacer les filtres"),
    ("Cập nhật tất cả ({})", "Update all ({})", "Tout mettre à jour ({})"),
    ("Không có app nào khớp", "No matching apps", "Aucune appli ne correspond"),
    ("Chưa cài app nào.", "No apps installed yet.", "Aucune appli installée."),
    ("Mở Cửa hàng", "Open the Store", "Ouvrir la Boutique"),
    ("Danh mục trống.", "The catalog is empty.", "Le catalogue est vide."),
    ("Đang ngoại tuyến. Danh mục sẽ cập nhật khi có mạng; các app đã cài vẫn mở được.", "Offline. The catalog will update when you're back online; installed apps still open.", "Hors ligne. Le catalogue sera mis à jour au retour du réseau ; les applis installées s'ouvrent toujours."),
    ("Xem chi tiết", "View details", "Voir les détails"),
    // Thẻ app
    ("Mở", "Open", "Ouvrir"),
    ("Mở (terminal)", "Open (terminal)", "Ouvrir (terminal)"),
    ("Cài", "Install", "Installer"),
    ("Cập nhật", "Update", "Mettre à jour"),
    ("Đã cài", "Installed", "Installée"),
    ("Đã bị thu hồi", "Revoked", "Révoquée"),
    ("Có bản {}", "Version {} available", "Version {} disponible"),
    ("Bản {}", "Version {}", "Version {}"),
    ("có bản cập nhật", "update available", "mise à jour disponible"),
    ("· chạy trong terminal", "· runs in a terminal", "· s'exécute dans un terminal"),
    ("Cập nhật lên {}", "Update to {}", "Mettre à jour vers {}"),
    ("Quay lại {}", "Roll back to {}", "Revenir à {}"),
    ("Xem mã nguồn", "View source code", "Voir le code source"),
    ("Gỡ...", "Uninstall...", "Désinstaller..."),
    ("Mã nguồn", "Source code", "Code source"),
    ("Mã nguồn (GitHub)", "Source code (GitHub)", "Code source (GitHub)"),
    ("Website", "Website", "Site web"),
    (
        "Phiên bản này đã bị thu hồi: {}. Hãy cập nhật, quay lại bản trước hoặc gỡ app.",
        "This version was revoked: {}. Update, roll back or uninstall the app.",
        "Cette version a été révoquée : {}. Mettez à jour, revenez à la version précédente ou désinstallez l'appli.",
    ),
    // Hộp thoại
    ("Cài {} {}?", "Install {} {}?", "Installer {} {} ?"),
    (
        "Dung lượng tải về: {} MB. Gói được kiểm tra chữ ký của F-Eco trước khi cài.",
        "Download size: {} MB. The package's F-Eco signature is checked before installing.",
        "Taille du téléchargement : {} Mo. La signature F-Eco du paquet est vérifiée avant l'installation.",
    ),
    ("App này chạy với quyền như mọi ứng dụng khác trên máy bạn", "This app runs with the same permissions as any other app on your computer", "Cette appli s'exécute avec les mêmes droits que toute autre application de votre ordinateur"),
    ("Hủy", "Cancel", "Annuler"),
    ("Gỡ {}?", "Uninstall {}?", "Désinstaller {} ?"),
    ("Gỡ", "Uninstall", "Désinstaller"),
    ("Xóa cả dữ liệu cục bộ của app trên máy này", "Also delete the app's local data on this computer", "Supprimer aussi les données locales de l'appli sur cet ordinateur"),
    ("Gỡ app không thu hồi quyền dữ liệu bạn đã cấp cho app.", "Uninstalling doesn't revoke the data access you granted the app.", "La désinstallation ne révoque pas l'accès aux données accordé à l'appli."),
    ("Quản lý quyền ở trang Ứng dụng đã kết nối", "Manage access on the Connected apps page", "Gérer les accès sur la page Applis connectées"),
    // Trạng thái và thông báo
    ("Đang đăng nhập lại và đồng bộ...", "Signing in again and syncing...", "Reconnexion et synchronisation..."),
    ("Đang chờ đăng nhập trong trình duyệt...", "Waiting for sign-in in your browser...", "En attente de la connexion dans le navigateur..."),
    ("Xin chào {}", "Hello {}", "Bonjour {}"),
    ("Đang đăng xuất...", "Signing out...", "Déconnexion..."),
    ("Đã đăng xuất", "Signed out", "Déconnecté"),
    ("Đang đồng bộ...", "Syncing...", "Synchronisation..."),
    ("Đang cài {} {}...", "Installing {} {}...", "Installation de {} {}..."),
    ("Đã cài {} {}", "Installed {} {}", "{} {} installée"),
    ("Đang cập nhật...", "Updating...", "Mise à jour..."),
    ("không có bản cập nhật", "no update available", "aucune mise à jour disponible"),
    ("Đã cập nhật {} lên {}", "Updated {} to {}", "{} mise à jour vers {}"),
    ("Đang cập nhật tất cả...", "Updating everything...", "Mise à jour de tout..."),
    ("Đang quay lại bản trước...", "Rolling back...", "Retour à la version précédente..."),
    ("Đã quay lại {} {}", "Rolled back to {} {}", "Retour à {} {}"),
    ("Đang gỡ...", "Uninstalling...", "Désinstallation..."),
    (
        "Đã gỡ {}. Quyền dữ liệu bạn đã cấp cho app vẫn còn; thu hồi ở trang \"Ứng dụng đã kết nối\".",
        "Uninstalled {}. The data access you granted the app remains; revoke it on the \"Connected apps\" page.",
        "{} désinstallée. L'accès aux données accordé à l'appli reste actif ; révoquez-le sur la page « Applis connectées ».",
    ),
    ("Không mở được trình duyệt: {}", "Couldn't open the browser: {}", "Impossible d'ouvrir le navigateur : {}"),
    ("Đang mở...", "Opening...", "Ouverture..."),
    ("Đã mở {} {}{}", "Opened {} {}{}", "{} {} ouverte{}"),
    (" (không lấy được mã đăng nhập: {})", " (couldn't get a sign-in code: {})", " (impossible d'obtenir un code de connexion : {})"),
    (" (chưa đăng nhập: app sẽ tự yêu cầu đăng nhập)", " (not signed in: the app will ask you to sign in)", " (non connecté : l'appli vous demandera de vous connecter)"),
    ("không mở được {}: {}", "couldn't open {}: {}", "impossible d'ouvrir {} : {}"),
    ("không tìm thấy phông chữ hệ thống; đặt FECO_LAUNCHER_FONT=<đường dẫn .ttf>", "no system font found; set FECO_LAUNCHER_FONT=<path to a .ttf>", "aucune police système trouvée ; définissez FECO_LAUNCHER_FONT=<chemin d'un .ttf>"),
    ("không đọc được root.json tin cậy {}: {}", "couldn't read the trusted root.json {}: {}", "impossible de lire le root.json de confiance {} : {}"),
    ("không mở được API cục bộ: {}", "couldn't start the local API: {}", "impossible de démarrer l'API locale : {}"),
    ("Mọi app đã ở bản mới nhất", "All apps are up to date", "Toutes les applis sont à jour"),
    ("Đã cập nhật {} app", "Updated {} apps", "{} applis mises à jour"),
    ("Cập nhật lỗi (bản cũ vẫn dùng được): {}", "Update failed (the previous version still works): {}", "Échec de la mise à jour (l'ancienne version fonctionne toujours) : {}"),
    ("Không lưu được cài đặt: {}", "Couldn't save settings: {}", "Impossible d'enregistrer les paramètres : {}"),
    ("Chưa có thông báo nào. Đăng nhập để nhận thông báo về đơn hàng, thanh toán, cập nhật app.", "No notifications yet. Sign in to get notifications about orders, payments and app updates.", "Aucune notification. Connectez-vous pour être averti des commandes, paiements et mises à jour."),
    // Cài đặt
    ("Cài đặt giao diện lưu trên máy này, không gửi lên máy chủ.", "Appearance settings are stored on this computer and never sent to the server.", "Les paramètres d'apparence sont enregistrés sur cet ordinateur et jamais envoyés au serveur."),
    ("Giao diện", "Appearance", "Apparence"),
    ("Ngôn ngữ", "Language", "Langue"),
    ("Phong cách", "Style", "Style"),
    ("Phong cách này dùng bảng màu riêng; chủ đề và màu nhấn không áp dụng.", "This style has its own palette; theme and accent color don't apply.", "Ce style a sa propre palette ; le thème et la couleur d'accent ne s'appliquent pas."),
    ("Chủ đề", "Theme", "Thème"),
    ("Màu nhấn", "Accent color", "Couleur d'accent"),
    ("Cỡ chữ", "Text size", "Taille du texte"),
    ("Bố cục mặc định", "Default layout", "Disposition par défaut"),
    ("Cỡ ô lưới", "Tile size", "Taille des tuiles"),
    ("Trang mở đầu", "Start page", "Page d'accueil"),
    ("Hiển thị", "Display", "Affichage"),
    ("Hiện mô tả ngắn của app", "Show short app descriptions", "Afficher les descriptions courtes"),
    ("Chế độ gọn (ít khoảng trống)", "Compact mode (less spacing)", "Mode compact (moins d'espace)"),
    ("Khôi phục mặc định", "Restore defaults", "Rétablir les valeurs par défaut"),
    ("Tự động cập nhật app (kiểm tra khi mở và mỗi 6 giờ)", "Update apps automatically (checks at startup and every 6 hours)", "Mettre à jour les applis automatiquement (au démarrage et toutes les 6 heures)"),
    ("Kiểm tra cập nhật ngay", "Check for updates now", "Rechercher des mises à jour"),
    ("Tài khoản", "Account", "Compte"),
    ("Đã đăng nhập: {}", "Signed in as {}", "Connecté en tant que {}"),
    ("Ứng dụng đã kết nối", "Connected apps", "Applis connectées"),
    ("Giới thiệu", "About", "À propos"),
    ("Phiên bản", "Version", "Version"),
    ("Nền tảng", "Platform", "Plateforme"),
    ("Thư mục dữ liệu", "Data folder", "Dossier de données"),
    ("{}. Mọi gói cài đặt được kiểm tra chữ ký của F-Eco trước khi cài.", "{}. Every package's F-Eco signature is checked before installing.", "{}. La signature F-Eco de chaque paquet est vérifiée avant l'installation."),
    // Trang chi tiết
    ("App này không còn trong danh mục.", "This app is no longer in the catalog.", "Cette appli n'est plus dans le catalogue."),
    ("Phím mũi tên phải", "Right arrow key", "Flèche droite"),
    ("Phím mũi tên trái", "Left arrow key", "Flèche gauche"),
    ("‹ Thư viện", "‹ Library", "‹ Bibliothèque"),
    ("‹ Cửa hàng", "‹ Store", "‹ Boutique"),
    ("Sau ›", "Next ›", "Suivante ›"),
    ("Sau: {} ›", "Next: {} ›", "Suivante : {} ›"),
    ("‹ Trước", "‹ Previous", "‹ Précédente"),
    ("‹ Trước: {}", "‹ Previous: {}", "‹ Précédente : {}"),
    ("Phiên bản đã cài bị thu hồi: {}", "The installed version was revoked: {}", "La version installée a été révoquée : {}"),
    ("Tính năng", "Features", "Fonctionnalités"),
    ("Có gì mới trong bản {}", "What's new in {}", "Nouveautés de la version {}"),
    ("Phát hành ngày {}", "Released on {}", "Publiée le {}"),
    ("Thông tin", "Information", "Informations"),
    ("Phiên bản mới nhất", "Latest version", "Dernière version"),
    ("Chưa cài", "Not installed", "Non installée"),
    ("Ngày phát hành", "Release date", "Date de publication"),
    ("Dung lượng tải về", "Download size", "Taille du téléchargement"),
    ("Chạy trên", "Runs on", "Fonctionne sur"),
    ("Nhà phát triển", "Developer", "Développeur"),
    ("Nhóm", "Category", "Catégorie"),
    ("Giấy phép", "License", "Licence"),
    ("Cách chạy", "How it runs", "Exécution"),
    ("Trong cửa sổ terminal", "In a terminal window", "Dans une fenêtre de terminal"),
    ("Cửa sổ riêng của app", "In its own window", "Dans sa propre fenêtre"),
    ("Tài khoản F-Eco", "F-Eco account", "Compte F-Eco"),
    ("Đăng nhập một lần qua F-Launcher", "Single sign-on through F-Launcher", "Authentification unique via F-Launcher"),
    ("Không dùng", "Not used", "Non utilisé"),
    ("{}. Gói được kiểm tra chữ ký của F-Eco trước khi cài.", "{}. The package's F-Eco signature is checked before installing.", "{}. La signature F-Eco du paquet est vérifiée avant l'installation."),
    // Sắp xếp (library.rs)
    ("Tên (A–Z)", "Name (A–Z)", "Nom (A–Z)"),
    ("Mở gần đây", "Recently opened", "Ouvertes récemment"),
    ("Cài gần đây", "Recently installed", "Installées récemment"),
    ("Dung lượng", "Size", "Taille"),
    // Nhóm app (library.rs)
    ("Năng suất", "Productivity", "Productivité"),
    ("Công cụ", "Tools", "Outils"),
    ("Hệ thống", "System", "Système"),
    ("Lập trình", "Development", "Développement"),
    ("Giáo dục", "Education", "Éducation"),
    ("Trò chơi", "Games", "Jeux"),
    ("Đồ họa", "Graphics", "Graphisme"),
    ("Âm nhạc", "Music", "Musique"),
    ("Video", "Video", "Vidéo"),
    ("Kinh doanh", "Business", "Affaires"),
    ("Khác", "Other", "Autre"),
    // Cài đặt (prefs.rs)
    ("Theo hệ thống", "System", "Système"),
    ("Sáng", "Light", "Clair"),
    ("Tối", "Dark", "Sombre"),
    ("Xanh dương", "Blue", "Bleu"),
    ("Xanh ngọc", "Teal", "Sarcelle"),
    ("Xanh lá", "Green", "Vert"),
    ("Tím", "Purple", "Violet"),
    ("Hồng", "Pink", "Rose"),
    ("Cam", "Orange", "Orange"),
    ("Lưới", "Grid", "Grille"),
    ("Danh sách", "List", "Liste"),
    ("Nhỏ", "Small", "Petite"),
    ("Vừa", "Medium", "Moyenne"),
    ("Lớn", "Large", "Grande"),
    // Phong cách (style.rs)
    ("Chân phương", "Plain", "Sobre"),
    ("Trang nhã", "Elegant", "Élégant"),
    ("Phẳng, góc gần vuông; theo chủ đề và màu nhấn bạn chọn", "Flat with near-square corners; follows your theme and accent color", "Plat, coins presque carrés ; suit votre thème et votre couleur d'accent"),
    ("Nút và thẻ bo tròn, thoáng hơn; theo chủ đề và màu nhấn bạn chọn", "Rounded buttons and cards, more breathing room; follows your theme and accent color", "Boutons et cartes arrondis, plus aéré ; suit votre thème et votre couleur d'accent"),
    ("Cảm hứng từ bảng màu bản phác thảo: giấy da, nâu sepia, đất nung", "Inspired by a sketchbook palette: parchment, sepia, terracotta", "Inspiré d'une palette de croquis : parchemin, sépia, terre cuite"),
    ("Cảm hứng từ bảng màu bích họa: vữa sáng, xanh lam đá, đỏ đất", "Inspired by a fresco palette: pale plaster, stone blue, earth red", "Inspiré d'une palette de fresque : enduit clair, bleu de pierre, rouge de terre"),
    ("Cảm hứng từ bảng màu trời đêm: xanh thẫm, vàng crôm, xanh coban", "Inspired by a night-sky palette: deep blue, chrome yellow, cobalt", "Inspiré d'une palette de ciel nocturne : bleu profond, jaune de chrome, cobalt"),
    ("Cảm hứng từ bảng màu mặt nước: lục nhạt, tím oải hương, hồng phấn", "Inspired by a water-garden palette: pale green, lavender, powder pink", "Inspiré d'une palette de jardin d'eau : vert pâle, lavande, rose poudré"),
    ("Cảm hứng từ thời kỳ Xanh và tranh lập thể: xanh lam, góc vuông, viền đậm", "Inspired by the Blue Period and cubism: blues, square corners, bold outlines", "Inspiré de la période bleue et du cubisme : bleus, coins carrés, contours marqués"),
    ("Cảm hứng từ bảng màu sơn dầu: trắng ngà, lục nhạt, vàng hoa huệ", "Inspired by an oil-painting palette: ivory, pale green, lily yellow", "Inspiré d'une palette de peinture à l'huile : ivoire, vert pâle, jaune de lys"),
    ("Cảm hứng từ bảng màu phố cũ: vàng đất, xám ngói, nâu son", "Inspired by an old-quarter palette: ochre, roof-tile grey, rust brown", "Inspiré d'une palette de vieux quartier : ocre, gris tuile, brun rouille"),
    ("Cảm hứng từ bảng màu sơn mài: đen then, son đỏ, vàng thếp", "Inspired by a lacquer palette: lacquer black, cinnabar red, gold leaf", "Inspiré d'une palette de laque : noir de laque, rouge cinabre, feuille d'or"),
];

#[cfg(test)]
mod tests {
    use super::*;

    fn placeholders(s: &str) -> usize {
        s.matches("{}").count()
    }

    #[test]
    fn translate_and_fill() {
        assert_eq!(translate(Language::En, "Thư viện"), "Library");
        assert_eq!(translate(Language::Fr, "Cài"), "Installer");
        assert_eq!(translate(Language::Vi, "Thư viện"), "Thư viện");
        // Chưa có trong bảng: giữ tiếng Việt.
        assert_eq!(
            translate(Language::En, "Chuỗi chưa dịch"),
            "Chuỗi chưa dịch"
        );
        assert_eq!(
            fill(
                translate(Language::Fr, "Đã cài {} {}"),
                &[&"kibi", &"0.3.3"]
            ),
            "kibi 0.3.3 installée"
        );
        assert_eq!(fill("{} và {}", &[&1]), "1 và ");
        for lang in Language::ALL {
            assert!(!lang.native_name().is_empty());
        }
    }

    /// Scenario "Đổi sang tiếng Pháp" (phần chuỗi).
    #[test]
    fn french_labels_from_the_spec() {
        assert_eq!(translate(Language::Fr, "Mọi nhóm"), "Toutes les catégories");
        assert_eq!(translate(Language::Fr, "Năng suất"), "Productivité");
        for (vi, en) in [
            ("Thư viện", "Library"),
            ("Cửa hàng", "Store"),
            ("Thông báo", "Notifications"),
            ("Cài đặt", "Settings"),
        ] {
            assert_eq!(translate(Language::En, vi), en);
        }
    }

    #[test]
    fn table_is_consistent() {
        let mut seen = std::collections::HashSet::new();
        for (vi, en, fr) in TABLE {
            assert!(seen.insert(*vi), "trùng khóa: {vi}");
            assert!(!en.is_empty() && !fr.is_empty(), "thiếu bản dịch: {vi}");
            let n = placeholders(vi);
            assert_eq!(placeholders(en), n, "số {{}} của bản tiếng Anh: {vi}");
            assert_eq!(placeholders(fr), n, "số {{}} của bản tiếng Pháp: {vi}");
        }
    }

    /// Chuỗi trong lời gọi `t("…")` / `tf("…", …)` của một tệp mã.
    fn keys_in(src: &str) -> Vec<String> {
        let mut out = Vec::new();
        for marker in ["t(\"", "tf(\""] {
            let mut rest = src;
            while let Some(i) = rest.find(marker) {
                // Bỏ qua tên hàm khác kết thúc bằng t( (ví dụ "print(\"").
                let before = rest[..i].chars().last();
                rest = &rest[i + marker.len()..];
                if before.is_some_and(|c| c.is_alphanumeric() || c == '_') {
                    continue;
                }
                let mut key = String::new();
                let mut chars = rest.chars();
                while let Some(c) = chars.next() {
                    match c {
                        '\\' => {
                            if let Some(n) = chars.next() {
                                key.push(n);
                            }
                        }
                        '"' => break,
                        c => key.push(c),
                    }
                }
                out.push(key);
            }
        }
        out
    }

    const SOURCES: [(&str, &str); 4] = [
        ("main.rs", include_str!("main.rs")),
        ("library.rs", include_str!("library.rs")),
        ("prefs.rs", include_str!("prefs.rs")),
        ("style.rs", include_str!("style.rs")),
    ];

    /// Scenario "Thiếu bản dịch": mọi chuỗi gọi qua `t`/`tf` đều có trong bảng.
    #[test]
    fn every_ui_string_has_translations() {
        let mut missing = Vec::new();
        let mut count = 0;
        for (file, src) in SOURCES {
            for key in keys_in(src) {
                count += 1;
                if !table().contains_key(key.as_str()) {
                    missing.push(format!("{file}: {key}"));
                }
            }
        }
        assert!(count > 100, "chỉ tìm thấy {count} chuỗi");
        assert!(missing.is_empty(), "chưa dịch:\n{}", missing.join("\n"));
    }

    /// Không còn chuỗi có dấu tiếng Việt nằm ngoài `t`/`tf` trong mã giao diện (trừ nhật ký
    /// `eprintln!`, `expect` và chú thích). Lời gọi có thể trải qua nhiều dòng sau `cargo fmt`.
    #[test]
    fn no_untranslated_literals_in_main() {
        let src = include_str!("main.rs");
        let code = &src[..src.find("#[cfg(test)]").unwrap_or(src.len())];
        let allowed = |c: char| c.is_ascii() || "‹›•·–".contains(c);
        let mut offenders = Vec::new();
        let mut i = 0;
        let bytes = code.as_bytes();
        while let Some(off) = code[i..].find('"') {
            let start = i + off;
            let line_start = code[..start].rfind('\n').map_or(0, |p| p + 1);
            let line_end = code[start..].find('\n').map_or(code.len(), |p| start + p);
            let line = &code[line_start..line_end];
            let trimmed = line.trim_start();
            if trimmed.starts_with("//") || line.contains("eprintln!") || line.contains(".expect(")
            {
                i = line_end;
                continue;
            }
            // Chuỗi ký tự '"'.
            if start > 0 && bytes[start - 1] == b'\'' {
                i = start + 1;
                continue;
            }
            let mut end = start + 1;
            while end < code.len() {
                match bytes[end] {
                    b'\\' => end += 2,
                    b'"' => break,
                    _ => end += 1,
                }
            }
            let lit = &code[start + 1..end];
            let before = code[..start].trim_end();
            // `t(` / `tf(` là lời gọi riêng, không phải đuôi của tên khác (ví dụ `hint_text(`).
            let wrapped = ["t(", "tf("].iter().any(|f| {
                before.ends_with(f)
                    && !before[..before.len() - f.len()]
                        .ends_with(|c: char| c.is_alphanumeric() || c == '_')
            });
            if !wrapped && !lit.chars().all(allowed) {
                offenders.push(lit.to_owned());
            }
            i = end + 1;
        }
        assert!(
            offenders.is_empty(),
            "chuỗi chưa đi qua t():\n{}",
            offenders.join("\n")
        );
    }
}
