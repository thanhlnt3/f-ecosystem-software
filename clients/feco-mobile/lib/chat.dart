/// Tin nhắn trên điện thoại (thay đổi chat-and-source-scan, spec mobile-launcher "Tin nhắn trên
/// điện thoại"): luồng tin của một hội thoại, hỏi tin mới mỗi 5 giây (design C3), thông báo lỗi.
library;

import 'dart:async';

import 'package:flutter/foundation.dart';

import 'api.dart';
import 'i18n.dart';
import 'models.dart';

/// Khoảng hỏi tin mới khi đang mở hội thoại (C3).
const chatPollInterval = Duration(seconds: 5);

/// Mã lỗi khi không gọi được máy chủ (mất mạng, trả lời hỏng).
const chatNetworkError = 'network';

const _knownErrors = {
  'chat_blocked',
  'chat_too_long',
  'chat_empty',
  'chat_rate_limited',
  'chat_forbidden',
  'chat_not_found',
  'chat_not_developer',
  'chat_self',
};

/// Khóa chuỗi (i18n) của mã lỗi [code]; mã lạ dùng thông báo chung.
String chatErrorKey(String code) {
  if (_knownErrors.contains(code)) return 'chat.err.$code';
  if (code == chatNetworkError) return 'error_network';
  if (code == 'not_signed_in') return 'need_sign_in';
  return 'chat.err.generic';
}

/// Thông báo lỗi dễ hiểu theo ngôn ngữ [lang].
String chatErrorMessage(Lang lang, String code) => tr(lang, chatErrorKey(code));

/// Mã lỗi của một ngoại lệ khi gọi API tin nhắn.
String chatErrorCode(Object e) => e is ApiException ? e.code : chatNetworkError;

/// Cỡ tệp: B, KB, MB (KB = 1024 B).
String chatFileSize(int bytes, Lang lang) {
  if (bytes < 1024) return tr(lang, 'chat.size_b', {'n': bytes});
  if (bytes < 1024 * 1024) {
    return tr(lang, 'chat.size_kb', {'n': (bytes / 1024).ceil()});
  }
  final mb = bytes / (1024 * 1024);
  final text = mb.toStringAsFixed(1);
  return tr(lang, 'chat.size_mb', {
    'n': lang == Lang.vi || lang == Lang.fr ? text.replaceAll('.', ',') : text,
  });
}

String _two(int n) => n.toString().padLeft(2, '0');

/// Thời điểm của tin: giờ:phút trong ngày hôm nay, thêm ngày/tháng với ngày khác, thêm năm với
/// năm khác. Giờ địa phương.
String chatTime(DateTime? at, Lang lang, {DateTime? now}) {
  if (at == null) return '';
  final t = at.toLocal();
  final n = (now ?? DateTime.now()).toLocal();
  final hm = '${_two(t.hour)}:${_two(t.minute)}';
  if (t.year == n.year && t.month == n.month && t.day == n.day) return hm;
  final dm = switch (lang) {
    Lang.en || Lang.ja => '${t.month}/${t.day}',
    Lang.vi || Lang.fr => '${_two(t.day)}/${_two(t.month)}',
  };
  if (t.year != n.year) {
    return switch (lang) {
      Lang.en || Lang.ja => '${t.year}/$dm $hm',
      Lang.vi || Lang.fr => '$dm/${t.year} $hm',
    };
  }
  return '$dm $hm';
}

/// Luồng tin của một hội thoại đang mở: tải tin, hỏi tin mới định kỳ, gửi tin, đánh dấu đã đọc.
class ChatThread extends ChangeNotifier {
  ChatThread({required this.backend, required this._conversation, this.onRead});

  final Backend backend;

  /// Gọi sau khi đánh dấu đã đọc (làm mới số chưa đọc, danh sách hội thoại).
  final Future<void> Function()? onRead;

  Conversation _conversation;
  Conversation get conversation => _conversation;

  List<ChatMessage> messages = const [];
  bool loading = true;
  bool sending = false;

  /// Mã lỗi gần nhất (hiện bằng [chatErrorMessage]); null khi không có lỗi.
  String? error;

  Timer? _timer;
  bool _polling = false;
  bool _disposed = false;
  int _readUpTo = 0;

  int get lastId => messages.isEmpty ? 0 : messages.last.id;

  /// Đang hỏi tin mới định kỳ.
  bool get polling => _timer?.isActive ?? false;

  /// Khách hàng bị chặn trong hội thoại `direct`: không gửi được.
  bool get canSend => !_conversation.customerBlocked;

  /// Tải luồng tin rồi hỏi tin mới mỗi [interval].
  Future<void> start({Duration interval = chatPollInterval}) {
    _timer?.cancel();
    _timer = Timer.periodic(interval, (_) => poll());
    return load();
  }

  /// Ngừng hỏi tin mới (khi rời hội thoại).
  void stop() {
    _timer?.cancel();
    _timer = null;
  }

  void _notify() {
    if (!_disposed) notifyListeners();
  }

  Future<void> load() async {
    loading = true;
    _notify();
    try {
      messages = mergeMessages(
        const [],
        await backend.messages(_conversation.id),
      );
      error = null;
    } on Exception catch (e) {
      error = chatErrorCode(e);
    } finally {
      loading = false;
      _notify();
    }
    await _markRead();
  }

  /// Mốc `after` khi hỏi tin mới: tin cuối, hoặc trước tin có đính kèm còn đang quét (để cập nhật
  /// trạng thái quét).
  int get pollAfter {
    var after = lastId;
    for (final m in messages) {
      if (m.attachment?.status == AttachmentStatus.pending &&
          m.id - 1 < after) {
        after = m.id - 1;
      }
    }
    return after;
  }

  /// Hỏi tin mới (`?after=`), gộp theo `id`, đánh dấu đã đọc khi có tin mới.
  Future<void> poll() async {
    if (_polling || _disposed || loading) return;
    _polling = true;
    try {
      final fresh = await backend.messages(_conversation.id, after: pollAfter);
      if (_disposed) return;
      if (fresh.isNotEmpty) {
        messages = mergeMessages(messages, fresh);
        _notify();
      }
      await _markRead();
    } on Exception {
      // Lần sau hỏi lại; không che luồng tin vì một lần mất mạng.
    } finally {
      _polling = false;
    }
  }

  /// Đánh dấu đã đọc tới tin cuối khi có tin của bên kia chưa đánh dấu.
  Future<void> _markRead() async {
    final last = lastId;
    if (_disposed || last <= _readUpTo) return;
    final unseen = messages.any((m) => m.id > _readUpTo && !m.mine);
    if (!unseen) {
      _readUpTo = last;
      return;
    }
    try {
      await backend.markRead(_conversation.id, last);
      _readUpTo = last;
      await onRead?.call();
    } on Exception {
      // Thử lại ở lần hỏi sau.
    }
  }

  /// Gửi tin [text]. Trả null khi đã gửi, hoặc mã lỗi (kiểm trước ở máy, rồi mã của máy chủ;
  /// `busy` khi tin trước còn đang gửi).
  Future<String?> send(String text) async {
    if (sending) return 'busy';
    final invalid = validateChatBody(text);
    if (invalid != null) {
      error = invalid;
      _notify();
      return invalid;
    }
    if (!canSend) {
      error = 'chat_blocked';
      _notify();
      return error;
    }
    sending = true;
    error = null;
    _notify();
    try {
      final m = await backend.sendMessage(_conversation.id, text.trim());
      messages = mergeMessages(messages, [m]);
      return null;
    } on Exception catch (e) {
      error = chatErrorCode(e);
      if (error == 'chat_blocked') {
        _conversation = _conversation.copyWith(blocked: true);
      }
      return error;
    } finally {
      sending = false;
      _notify();
    }
  }

  @override
  void dispose() {
    _disposed = true;
    stop();
    super.dispose();
  }
}
