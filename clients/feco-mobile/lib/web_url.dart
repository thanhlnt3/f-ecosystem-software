/// Địa chỉ app web được phép mở (spec mobile-launcher, "Cửa hàng trên điện thoại"): HTTPS với tên
/// máy bất kỳ, hoặc HTTP chỉ với máy dev (`localhost`, `127.0.0.1`, `10.0.2.2` của Android
/// emulator); không có tên đăng nhập hay mật khẩu. Cùng quy tắc với `feco_tuf::repo::valid_web_url`.
library;

const _devHosts = {'localhost', '127.0.0.1', '10.0.2.2'};

bool validWebUrl(String? raw) {
  if (raw == null || raw.trim().isEmpty) return false;
  final Uri u;
  try {
    u = Uri.parse(raw.trim());
  } on FormatException {
    return false;
  }
  if (!u.hasScheme || u.host.isEmpty || u.userInfo.isNotEmpty) return false;
  return switch (u.scheme) {
    'https' => true,
    'http' => _devHosts.contains(u.host),
    _ => false,
  };
}

/// Trên Android emulator, `localhost` là chính emulator: đổi sang `10.0.2.2` (máy dev) khi máy chủ
/// cũng chạy ở đó.
Uri forDevice(Uri url, Uri server) {
  if (url.scheme == 'http' &&
      (url.host == 'localhost' || url.host == '127.0.0.1') &&
      server.host == '10.0.2.2') {
    return url.replace(host: '10.0.2.2');
  }
  return url;
}
