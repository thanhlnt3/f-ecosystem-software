/// Giao diện tab Tin nhắn (spec mobile-launcher, "Tin nhắn trên điện thoại"): danh sách hội thoại
/// và luồng tin với ô soạn tin. Đính kèm chỉ xem; gửi tệp qua web.
library;

import 'package:material_ui/material_ui.dart';

import 'app_state.dart';
import 'chat.dart';
import 'models.dart';

/// Danh sách hội thoại. Kéo xuống để làm mới số chưa đọc và danh sách (C3).
class MessagesPage extends StatelessWidget {
  const MessagesPage({super.key, required this.state});
  final AppState state;

  @override
  Widget build(BuildContext context) {
    final s = state;
    return RefreshIndicator(
      onRefresh: s.refreshChat,
      child: ListView(
        key: const Key('conversation-list'),
        physics: const AlwaysScrollableScrollPhysics(),
        children: [
          if (s.chatError != null)
            ListTile(
              key: const Key('chat-list-error'),
              leading: Icon(
                Icons.error_outline,
                color: Theme.of(context).colorScheme.error,
              ),
              title: Text(chatErrorMessage(s.lang, s.chatError!)),
              trailing: TextButton(
                onPressed: s.refreshChat,
                child: Text(s.t('retry')),
              ),
            )
          else if (s.conversations.isEmpty)
            Padding(
              padding: const EdgeInsets.all(24),
              child: Text(
                s.t('chat.no_conversations'),
                textAlign: TextAlign.center,
              ),
            ),
          for (final c in s.conversations)
            ConversationTile(state: s, conversation: c),
        ],
      ),
    );
  }
}

class ConversationTile extends StatelessWidget {
  const ConversationTile({
    super.key,
    required this.state,
    required this.conversation,
  });
  final AppState state;
  final Conversation conversation;

  @override
  Widget build(BuildContext context) {
    final s = state;
    final c = conversation;
    final unread = c.unread > 0;
    final theme = Theme.of(context);
    return ListTile(
      key: Key('conv-${c.id}'),
      leading: CircleAvatar(
        child: Icon(
          c.kind == ConversationKind.project
              ? Icons.work_outline
              : Icons.person_outline,
        ),
      ),
      title: Text(
        c.title.isEmpty ? c.peerName : c.title,
        maxLines: 1,
        overflow: TextOverflow.ellipsis,
        style: TextStyle(
          fontWeight: unread ? FontWeight.w600 : FontWeight.normal,
        ),
      ),
      subtitle: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        mainAxisSize: MainAxisSize.min,
        children: [
          Text(
            [
              if (c.kind == ConversationKind.project) s.t('chat.project'),
              c.peerName,
            ].join(' · '),
            maxLines: 1,
            overflow: TextOverflow.ellipsis,
            style: theme.textTheme.labelMedium,
          ),
          if (c.lastMessage != null)
            Text(
              c.lastMessage!,
              maxLines: 1,
              overflow: TextOverflow.ellipsis,
              style: TextStyle(
                fontWeight: unread ? FontWeight.w600 : FontWeight.normal,
              ),
            ),
        ],
      ),
      trailing: Column(
        mainAxisSize: MainAxisSize.min,
        crossAxisAlignment: CrossAxisAlignment.end,
        children: [
          Text(
            chatTime(c.lastMessageAt, s.lang),
            style: theme.textTheme.labelSmall,
          ),
          if (unread)
            Padding(
              padding: const EdgeInsets.only(top: 4),
              child: Semantics(
                label: s.t('chat.unread', {'n': c.unread}),
                child: Badge(
                  key: Key('conv-unread-${c.id}'),
                  label: Text(c.unread > 99 ? '99+' : '${c.unread}'),
                ),
              ),
            ),
        ],
      ),
      onTap: () => openThread(context, s, c),
    );
  }
}

/// Mở luồng tin; rời luồng thì làm mới số chưa đọc và danh sách.
Future<void> openThread(
  BuildContext context,
  AppState s,
  Conversation c,
) async {
  await Navigator.of(context).push(
    MaterialPageRoute<void>(
      builder: (_) => ThreadPage(state: s, conversation: c),
    ),
  );
  await s.refreshChat();
}

/// Luồng tin của một hội thoại: hỏi tin mới mỗi [pollInterval] khi đang mở (C3).
class ThreadPage extends StatefulWidget {
  const ThreadPage({
    super.key,
    required this.state,
    required this.conversation,
    this.pollInterval = chatPollInterval,
  });
  final AppState state;
  final Conversation conversation;
  final Duration pollInterval;

  @override
  State<ThreadPage> createState() => _ThreadPageState();
}

class _ThreadPageState extends State<ThreadPage> {
  late final ChatThread _thread;
  final _input = TextEditingController();

  @override
  void initState() {
    super.initState();
    _thread = ChatThread(
      backend: widget.state.backend,
      conversation: widget.conversation,
      onRead: widget.state.refreshChat,
    );
    _thread.start(interval: widget.pollInterval);
  }

  @override
  void dispose() {
    _thread.dispose(); // hủy Timer hỏi tin mới
    _input.dispose();
    super.dispose();
  }

  Future<void> _send() async {
    final err = await _thread.send(_input.text);
    if (err == null && mounted) _input.clear();
  }

  @override
  Widget build(BuildContext context) => ListenableBuilder(
    listenable: Listenable.merge([widget.state, _thread]),
    builder: (context, _) {
      final s = widget.state;
      final t = _thread;
      final c = t.conversation;
      final theme = Theme.of(context);
      final msgs = t.messages;
      return Scaffold(
        appBar: AppBar(
          title: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Text(
                c.title.isEmpty ? c.peerName : c.title,
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
              ),
              if (c.title.isNotEmpty && c.title != c.peerName)
                Text(c.peerName, style: theme.textTheme.labelMedium),
            ],
          ),
          actions: [
            if (c.webUrl != null)
              IconButton(
                key: const Key('chat-open-web'),
                tooltip: s.t('chat.open_web'),
                icon: const Icon(Icons.open_in_new),
                onPressed: () => s.openWeb(c.webUrl),
              ),
          ],
        ),
        body: SafeArea(
          child: Column(
            children: [
              if (t.loading) const LinearProgressIndicator(),
              Expanded(
                child: msgs.isEmpty && !t.loading
                    ? Center(child: Text(s.t('chat.no_messages')))
                    : ListView.builder(
                        key: const Key('chat-thread'),
                        // Tin mới nhất ở dưới cùng, luôn trong tầm nhìn.
                        reverse: true,
                        padding: const EdgeInsets.all(12),
                        itemCount: msgs.length,
                        itemBuilder: (_, i) => ChatBubble(
                          state: s,
                          message: msgs[msgs.length - 1 - i],
                        ),
                      ),
              ),
              if (c.customerBlocked)
                _Notice(
                  key: const Key('chat-blocked'),
                  icon: Icons.block,
                  text: s.t('chat.blocked_customer'),
                )
              else if (c.blocked)
                _Notice(icon: Icons.block, text: s.t('chat.blocked_developer')),
              if (t.error != null)
                Padding(
                  padding: const EdgeInsets.fromLTRB(16, 4, 16, 0),
                  child: Text(
                    chatErrorMessage(s.lang, t.error!),
                    key: const Key('chat-error'),
                    style: TextStyle(color: theme.colorScheme.error),
                  ),
                ),
              Padding(
                padding: const EdgeInsets.fromLTRB(4, 4, 8, 8),
                child: Row(
                  crossAxisAlignment: CrossAxisAlignment.end,
                  children: [
                    if (c.webUrl != null)
                      IconButton(
                        key: const Key('chat-attach-web'),
                        tooltip: s.t('chat.attach_on_web'),
                        icon: const Icon(Icons.attach_file),
                        onPressed: () => s.openWeb(c.webUrl),
                      ),
                    Expanded(
                      child: TextField(
                        key: const Key('chat-input'),
                        controller: _input,
                        enabled: t.canSend,
                        minLines: 1,
                        maxLines: 5,
                        keyboardType: TextInputType.multiline,
                        textCapitalization: TextCapitalization.sentences,
                        decoration: InputDecoration(
                          hintText: s.t('chat.input_hint'),
                          border: const OutlineInputBorder(),
                          isDense: true,
                        ),
                      ),
                    ),
                    const SizedBox(width: 8),
                    FilledButton(
                      key: const Key('chat-send'),
                      onPressed: t.canSend && !t.sending ? _send : null,
                      child: Text(s.t('chat.send')),
                    ),
                  ],
                ),
              ),
            ],
          ),
        ),
      );
    },
  );
}

class _Notice extends StatelessWidget {
  const _Notice({super.key, required this.icon, required this.text});
  final IconData icon;
  final String text;

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    return Container(
      width: double.infinity,
      color: scheme.errorContainer,
      padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 8),
      child: Row(
        children: [
          Icon(icon, size: 18, color: scheme.onErrorContainer),
          const SizedBox(width: 8),
          Expanded(
            child: Text(text, style: TextStyle(color: scheme.onErrorContainer)),
          ),
        ],
      ),
    );
  }
}

/// Một tin: của mình bên phải, của bên kia bên trái; tin quản trị viên có viền và nhãn riêng; tin
/// hệ thống ở giữa, chữ nghiêng.
class ChatBubble extends StatelessWidget {
  const ChatBubble({super.key, required this.state, required this.message});
  final AppState state;
  final ChatMessage message;

  @override
  Widget build(BuildContext context) {
    final s = state;
    final m = message;
    final theme = Theme.of(context);
    final scheme = theme.colorScheme;
    final time = chatTime(m.at, s.lang);
    if (m.side == MessageSide.system) {
      return Padding(
        key: Key('msg-${m.id}'),
        padding: const EdgeInsets.symmetric(vertical: 8, horizontal: 24),
        child: Text(
          time.isEmpty ? m.body : '${m.body} · $time',
          textAlign: TextAlign.center,
          style: theme.textTheme.bodySmall?.copyWith(
            fontStyle: FontStyle.italic,
            color: scheme.onSurfaceVariant,
          ),
        ),
      );
    }
    final admin = m.side == MessageSide.admin && !m.mine;
    final bg = m.mine
        ? scheme.primaryContainer
        : admin
        ? scheme.tertiaryContainer
        : scheme.surfaceContainerHighest;
    final fg = m.mine
        ? scheme.onPrimaryContainer
        : admin
        ? scheme.onTertiaryContainer
        : scheme.onSurface;
    const r = Radius.circular(16);
    return Align(
      key: Key('msg-${m.id}'),
      alignment: m.mine ? Alignment.centerRight : Alignment.centerLeft,
      child: ConstrainedBox(
        constraints: BoxConstraints(
          maxWidth: MediaQuery.sizeOf(context).width * 0.8,
        ),
        child: Container(
          margin: const EdgeInsets.symmetric(vertical: 4),
          padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 8),
          decoration: BoxDecoration(
            color: bg,
            border: admin ? Border.all(color: scheme.tertiary) : null,
            borderRadius: BorderRadius.only(
              topLeft: r,
              topRight: r,
              bottomLeft: m.mine ? r : const Radius.circular(4),
              bottomRight: m.mine ? const Radius.circular(4) : r,
            ),
          ),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            mainAxisSize: MainAxisSize.min,
            children: [
              if (admin)
                Padding(
                  padding: const EdgeInsets.only(bottom: 2),
                  child: Row(
                    mainAxisSize: MainAxisSize.min,
                    children: [
                      Icon(Icons.verified_user, size: 14, color: fg),
                      const SizedBox(width: 4),
                      Text(
                        s.t('chat.side.admin'),
                        style: theme.textTheme.labelSmall?.copyWith(
                          color: fg,
                          fontWeight: FontWeight.w600,
                        ),
                      ),
                    ],
                  ),
                ),
              if (m.body.isNotEmpty) Text(m.body, style: TextStyle(color: fg)),
              if (m.attachment != null)
                AttachmentView(
                  key: Key('att-${m.id}'),
                  state: s,
                  messageId: m.id,
                  attachment: m.attachment!,
                ),
              if (time.isNotEmpty)
                Padding(
                  padding: const EdgeInsets.only(top: 2),
                  child: Text(
                    time,
                    style: theme.textTheme.labelSmall?.copyWith(
                      color: fg.withValues(alpha: 0.7),
                    ),
                  ),
                ),
            ],
          ),
        ),
      ),
    );
  }
}

/// Tệp đính kèm: tên, cỡ, trạng thái quét. Chỉ tệp sạch mở được (trình duyệt).
class AttachmentView extends StatelessWidget {
  const AttachmentView({
    super.key,
    required this.state,
    required this.messageId,
    required this.attachment,
  });
  final AppState state;
  final int messageId;
  final ChatAttachment attachment;

  @override
  Widget build(BuildContext context) {
    final s = state;
    final a = attachment;
    final scheme = Theme.of(context).colorScheme;
    final bad =
        a.status == AttachmentStatus.infected ||
        a.status == AttachmentStatus.error;
    final icon = switch (a.status) {
      AttachmentStatus.pending => Icons.hourglass_top,
      AttachmentStatus.clean => Icons.attach_file,
      AttachmentStatus.infected => Icons.block,
      AttachmentStatus.error => Icons.error_outline,
    };
    return Container(
      margin: const EdgeInsets.only(top: 6),
      padding: const EdgeInsets.all(8),
      decoration: BoxDecoration(
        color: scheme.surface.withValues(alpha: 0.6),
        borderRadius: BorderRadius.circular(8),
        border: Border.all(color: bad ? scheme.error : scheme.outlineVariant),
      ),
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          Icon(icon, size: 20, color: bad ? scheme.error : null),
          const SizedBox(width: 8),
          Flexible(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              mainAxisSize: MainAxisSize.min,
              children: [
                Text(
                  a.name,
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: const TextStyle(fontWeight: FontWeight.w600),
                ),
                Text(
                  '${chatFileSize(a.size, s.lang)} · '
                  '${s.t('chat.att.${a.status.name}')}',
                  key: Key('att-status-$messageId'),
                  style: TextStyle(
                    fontSize: 12,
                    color: bad ? scheme.error : null,
                  ),
                ),
              ],
            ),
          ),
          if (a.canOpen)
            TextButton(
              key: Key('att-open-$messageId'),
              onPressed: () => s.openWeb(a.url),
              child: Text(s.t('open')),
            ),
        ],
      ),
    );
  }
}
