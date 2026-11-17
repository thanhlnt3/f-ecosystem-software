/// Giao diện: năm tab (spec mobile-launcher, "Năm tab").
library;

import 'package:material_ui/material_ui.dart';

import 'app_state.dart';
import 'i18n.dart';
import 'models.dart';

/// Màu của logo F-Launcher.
const brandBlue = Color(0xFF1E88E5);
const brandOrange = Color(0xFFF37021);

class FecoApp extends StatelessWidget {
  const FecoApp({super.key, required this.state});
  final AppState state;

  @override
  Widget build(BuildContext context) => ListenableBuilder(
    listenable: state,
    builder: (context, _) => MaterialApp(
      title: 'F-Launcher',
      debugShowCheckedModeBanner: false,
      theme: ThemeData(
        colorScheme: ColorScheme.fromSeed(seedColor: brandBlue),
        useMaterial3: true,
      ),
      darkTheme: ThemeData(
        colorScheme: ColorScheme.fromSeed(
          seedColor: brandBlue,
          brightness: Brightness.dark,
        ),
        useMaterial3: true,
      ),
      home: HomePage(state: state),
    ),
  );
}

class HomePage extends StatefulWidget {
  const HomePage({super.key, required this.state});
  final AppState state;

  @override
  State<HomePage> createState() => _HomePageState();
}

class _HomePageState extends State<HomePage> {
  // Chưa đăng nhập thì mở ở Cửa hàng (xem được không cần đăng nhập).
  int _tab = 1;

  @override
  Widget build(BuildContext context) {
    final s = widget.state;
    final titles = [
      s.t('tab.library'),
      s.t('tab.store'),
      s.t('tab.wallet'),
      s.t('tab.notifications'),
      s.t('tab.account'),
    ];
    final unread = s.notifications.where((n) => !n.read).length;
    final pages = [
      _NeedSignIn(
        state: s,
        child: LibraryPage(state: s),
      ),
      StorePage(state: s),
      _NeedSignIn(
        state: s,
        child: WalletPage(state: s),
      ),
      _NeedSignIn(
        state: s,
        child: NotificationsPage(state: s),
      ),
      AccountPage(state: s),
    ];
    return Scaffold(
      appBar: AppBar(
        title: Text(titles[_tab]),
        actions: [
          if (s.config.demo)
            Padding(
              padding: const EdgeInsets.only(right: 12),
              child: Chip(label: Text(s.t('demo'))),
            ),
        ],
      ),
      body: SafeArea(
        child: Column(
          children: [
            if (s.busy) const LinearProgressIndicator(),
            if (s.error != null)
              MaterialBanner(
                content: Text(s.error!),
                actions: [
                  TextButton(onPressed: s.reload, child: Text(s.t('retry'))),
                ],
              ),
            Expanded(
              child: IndexedStack(index: _tab, children: pages),
            ),
          ],
        ),
      ),
      bottomNavigationBar: NavigationBar(
        selectedIndex: _tab,
        onDestinationSelected: (i) => setState(() => _tab = i),
        destinations: [
          NavigationDestination(
            key: const Key('tab-library'),
            icon: const Icon(Icons.apps_outlined),
            selectedIcon: const Icon(Icons.apps),
            label: titles[0],
          ),
          NavigationDestination(
            key: const Key('tab-store'),
            icon: const Icon(Icons.storefront_outlined),
            selectedIcon: const Icon(Icons.storefront),
            label: titles[1],
          ),
          NavigationDestination(
            key: const Key('tab-wallet'),
            icon: const Icon(Icons.account_balance_wallet_outlined),
            selectedIcon: const Icon(Icons.account_balance_wallet),
            label: titles[2],
          ),
          NavigationDestination(
            key: const Key('tab-notifications'),
            icon: Badge(
              isLabelVisible: unread > 0,
              label: Text(unread > 9 ? '9+' : '$unread'),
              child: const Icon(Icons.notifications_outlined),
            ),
            label: titles[3],
          ),
          NavigationDestination(
            key: const Key('tab-account'),
            icon: const Icon(Icons.person_outline),
            selectedIcon: const Icon(Icons.person),
            label: titles[4],
          ),
        ],
      ),
    );
  }
}

/// Trang cần đăng nhập: chưa đăng nhập thì hiện nút "Đăng nhập".
class _NeedSignIn extends StatelessWidget {
  const _NeedSignIn({required this.state, required this.child});
  final AppState state;
  final Widget child;

  @override
  Widget build(BuildContext context) {
    if (state.signedIn) return child;
    return Center(
      child: Padding(
        padding: const EdgeInsets.all(24),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            Image.asset('assets/f-launcher.png', width: 72, height: 72),
            const SizedBox(height: 16),
            Text(state.t('need_sign_in'), textAlign: TextAlign.center),
            const SizedBox(height: 16),
            FilledButton.icon(
              key: const Key('sign-in'),
              onPressed: state.signIn,
              icon: const Icon(Icons.login),
              label: Text(state.t('sign_in')),
            ),
          ],
        ),
      ),
    );
  }
}

/// Logo của app, hoặc ô chữ cái đầu.
class AppIcon extends StatelessWidget {
  const AppIcon({
    super.key,
    required this.state,
    required this.id,
    required this.name,
    this.hasIcon = false,
  });
  final AppState state;
  final String id;
  final String name;
  final bool hasIcon;

  @override
  Widget build(BuildContext context) {
    final url = hasIcon ? state.backend.iconUrl(id) : null;
    final initials = name.trim().isEmpty ? '?' : name.trim()[0].toUpperCase();
    final fallback = Container(
      width: 48,
      height: 48,
      alignment: Alignment.center,
      decoration: BoxDecoration(
        color: Color(0xFF000000 | (id.hashCode & 0x00FFFFFF))
            .withValues(alpha: 1),
        borderRadius: BorderRadius.circular(11),
      ),
      child: Text(
        initials,
        style: const TextStyle(
          color: Colors.white,
          fontSize: 22,
          fontWeight: FontWeight.w600,
        ),
      ),
    );
    if (url == null) return fallback;
    return ClipRRect(
      borderRadius: BorderRadius.circular(11),
      child: Image.network(
        url.toString(),
        width: 48,
        height: 48,
        errorBuilder: (_, _, _) => fallback,
      ),
    );
  }
}

class StorePage extends StatefulWidget {
  const StorePage({super.key, required this.state});
  final AppState state;

  @override
  State<StorePage> createState() => _StorePageState();
}

class _StorePageState extends State<StorePage> {
  String _query = '';
  String? _category;

  @override
  Widget build(BuildContext context) {
    final s = widget.state;
    final cats = {for (final a in s.catalog) a.category}.toList()..sort();
    final q = _query.trim().toLowerCase();
    final shown = s.catalog.where((a) {
      final hit =
          q.isEmpty ||
          a.name.toLowerCase().contains(q) ||
          a.summary.toLowerCase().contains(q) ||
          a.developer.toLowerCase().contains(q);
      return hit && (_category == null || a.category == _category);
    }).toList();
    return RefreshIndicator(
      onRefresh: s.reload,
      child: ListView(
        padding: const EdgeInsets.all(12),
        children: [
          TextField(
            key: const Key('store-search'),
            decoration: InputDecoration(
              prefixIcon: const Icon(Icons.search),
              hintText: s.t('search'),
              border: const OutlineInputBorder(),
            ),
            onChanged: (v) => setState(() => _query = v),
          ),
          const SizedBox(height: 8),
          SingleChildScrollView(
            scrollDirection: Axis.horizontal,
            child: Row(
              children: [
                ChoiceChip(
                  label: Text(s.t('all_categories')),
                  selected: _category == null,
                  onSelected: (_) => setState(() => _category = null),
                ),
                for (final c in cats)
                  Padding(
                    padding: const EdgeInsets.only(left: 6),
                    child: ChoiceChip(
                      label: Text(categoryLabel(s.lang, c)),
                      selected: _category == c,
                      onSelected: (_) => setState(() => _category = c),
                    ),
                  ),
              ],
            ),
          ),
          const SizedBox(height: 8),
          for (final a in shown) StoreTile(state: s, app: a),
        ],
      ),
    );
  }
}

class StoreTile extends StatelessWidget {
  const StoreTile({super.key, required this.state, required this.app});
  final AppState state;
  final CatalogApp app;

  @override
  Widget build(BuildContext context) {
    final s = state;
    final price = s.priceOf(app);
    final mustBuy = price > 0 && !s.owns(app.id);
    final actions = <Widget>[];
    if (mustBuy) {
      actions.add(
        FilledButton(
          key: Key('buy-${app.id}'),
          onPressed: s.signedIn
              ? () => showBuyDialog(context, s, app)
              : s.signIn,
          child: Text(s.t('buy', {'n': group(price, s.lang)})),
        ),
      );
    } else if (app.isWeb) {
      if (s.signedIn && !s.inLibrary(app.id)) {
        actions.add(
          OutlinedButton(
            key: Key('add-${app.id}'),
            onPressed: () => s.add(app),
            child: Text(s.t('add')),
          ),
        );
      }
      actions.add(
        FilledButton(
          key: Key('open-${app.id}'),
          onPressed: () => s.openWeb(app.webUrl),
          child: Text(s.t('open')),
        ),
      );
    }
    return Card(
      margin: const EdgeInsets.symmetric(vertical: 6),
      child: Padding(
        padding: const EdgeInsets.all(12),
        child: Row(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            AppIcon(state: s, id: app.id, name: app.name, hasIcon: app.hasIcon),
            const SizedBox(width: 12),
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(
                    app.name,
                    style: Theme.of(context).textTheme.titleMedium,
                  ),
                  Text(
                    [
                      if (app.isWeb) s.t('web_app'),
                      app.developer,
                      categoryLabel(s.lang, app.category),
                    ].join(' · '),
                    style: Theme.of(context).textTheme.bodySmall,
                  ),
                  if (app.summary.isNotEmpty)
                    Padding(
                      padding: const EdgeInsets.only(top: 4),
                      child: Text(
                        app.summary,
                        maxLines: 2,
                        overflow: TextOverflow.ellipsis,
                      ),
                    ),
                  const SizedBox(height: 6),
                  Text(
                    price > 0
                        ? s.t('coins', {'n': group(price, s.lang)})
                        : s.t('free'),
                    style: Theme.of(context).textTheme.labelMedium,
                  ),
                  if (!app.isWeb)
                    Padding(
                      key: Key('desktop-${app.id}'),
                      padding: const EdgeInsets.only(top: 4),
                      child: Row(
                        children: [
                          const Icon(Icons.computer, size: 16),
                          const SizedBox(width: 4),
                          Flexible(child: Text(s.t('desktop_only'))),
                        ],
                      ),
                    ),
                  if (actions.isNotEmpty)
                    Padding(
                      padding: const EdgeInsets.only(top: 8),
                      child: Wrap(spacing: 8, children: actions),
                    ),
                ],
              ),
            ),
          ],
        ),
      ),
    );
  }
}

/// Hộp thoại mua app (spec mobile-launcher, "Ví và mua app trên điện thoại").
Future<void> showBuyDialog(BuildContext context, AppState s, CatalogApp app) {
  final price = s.priceOf(app);
  final balance = s.wallet?.balance ?? 0;
  final short = shortfall(balance, price);
  return showDialog<void>(
    context: context,
    builder: (ctx) => AlertDialog(
      title: Text(s.t('confirm_buy_title', {'app': app.name})),
      content: Text(
        short > 0
            ? s.t('short', {'n': group(short, s.lang)})
            : s.t('confirm_buy_body', {
                'price': group(price, s.lang),
                'after': group(balance - price, s.lang),
              }),
        key: const Key('buy-dialog-text'),
      ),
      actions: [
        TextButton(
          onPressed: () => Navigator.pop(ctx),
          child: Text(s.t('cancel')),
        ),
        if (short > 0)
          FilledButton(
            key: const Key('topup'),
            onPressed: () {
              Navigator.pop(ctx);
              s.openPage('wallet');
            },
            child: Text(s.t('topup')),
          )
        else
          FilledButton(
            key: const Key('confirm-buy'),
            onPressed: () async {
              Navigator.pop(ctx);
              final r = await s.buy(app);
              if (!context.mounted) return;
              ScaffoldMessenger.of(context).showSnackBar(
                SnackBar(
                  content: Text(switch (r) {
                    Purchased() => s.t('bought', {'app': app.name}),
                    PurchaseFailed(message: final m) => m,
                  }),
                ),
              );
            },
            child: Text(s.t('buy', {'n': group(price, s.lang)})),
          ),
      ],
    ),
  );
}

class LibraryPage extends StatelessWidget {
  const LibraryPage({super.key, required this.state});
  final AppState state;

  @override
  Widget build(BuildContext context) {
    final s = state;
    if (s.library.isEmpty) {
      return Center(
        child: Padding(
          padding: const EdgeInsets.all(24),
          child: Text(s.t('library_empty'), textAlign: TextAlign.center),
        ),
      );
    }
    return RefreshIndicator(
      onRefresh: s.reload,
      child: ListView(
        padding: const EdgeInsets.all(12),
        children: [
          for (final i in s.library)
            Card(
              key: Key('library-${i.id}'),
              child: ListTile(
                leading: AppIcon(
                  state: s,
                  id: i.id,
                  name: i.name,
                  hasIcon: i.hasIcon,
                ),
                title: Text(i.name),
                subtitle: Wrap(
                  spacing: 6,
                  children: [
                    if (i.webUrl != null) Text(s.t('web_app')),
                    if (i.purchased) Text(s.t('purchased')),
                    if (i.updateAvailable)
                      Text(
                        s.t('update_available'),
                        style: const TextStyle(color: brandOrange),
                      ),
                    if (i.webUrl == null) Text(s.t('desktop_only')),
                  ],
                ),
                trailing: i.webUrl == null
                    ? null
                    : FilledButton(
                        onPressed: () => s.openWeb(i.webUrl),
                        child: Text(s.t('open')),
                      ),
              ),
            ),
        ],
      ),
    );
  }
}

class WalletPage extends StatelessWidget {
  const WalletPage({super.key, required this.state});
  final AppState state;

  @override
  Widget build(BuildContext context) {
    final s = state;
    final w = s.wallet;
    return ListView(
      padding: const EdgeInsets.all(16),
      children: [
        Card(
          child: Padding(
            padding: const EdgeInsets.all(20),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  s.t('balance'),
                  style: Theme.of(context).textTheme.labelLarge,
                ),
                Text(
                  s.t('coins', {'n': group(w?.balance ?? 0, s.lang)}),
                  key: const Key('wallet-balance'),
                  style: Theme.of(context).textTheme.headlineMedium,
                ),
                if (w != null)
                  Text(s.t('rate', {'vnd': group(w.rateVnd, s.lang)})),
                if (w?.frozen ?? false)
                  Padding(
                    padding: const EdgeInsets.only(top: 8),
                    child: Text(
                      s.t('wallet_frozen'),
                      style: TextStyle(
                        color: Theme.of(context).colorScheme.error,
                      ),
                    ),
                  ),
              ],
            ),
          ),
        ),
        const SizedBox(height: 12),
        Wrap(
          spacing: 8,
          children: [
            FilledButton.icon(
              onPressed: () => s.openPage('wallet'),
              icon: const Icon(Icons.add_card),
              label: Text(s.t('topup')),
            ),
            OutlinedButton.icon(
              onPressed: () => s.openPage('wallet#withdraw'),
              icon: const Icon(Icons.payments_outlined),
              label: Text(s.t('withdraw')),
            ),
          ],
        ),
        const SizedBox(height: 8),
        Text(
          s.t('wallet_web_hint'),
          style: Theme.of(context).textTheme.bodySmall,
        ),
      ],
    );
  }
}

class NotificationsPage extends StatelessWidget {
  const NotificationsPage({super.key, required this.state});
  final AppState state;

  @override
  Widget build(BuildContext context) {
    final s = state;
    if (s.notifications.isEmpty) {
      return Center(child: Text(s.t('no_notifications')));
    }
    return ListView(
      children: [
        for (final n in s.notifications)
          ListTile(
            leading: Icon(
              n.read ? Icons.notifications_none : Icons.notifications_active,
              color: n.read ? null : brandOrange,
            ),
            title: Text(
              n.title,
              style: TextStyle(
                fontWeight: n.read ? FontWeight.normal : FontWeight.w600,
              ),
            ),
            subtitle: Text(n.body),
            onTap: n.link.isEmpty ? null : () => s.openPage(n.link),
          ),
      ],
    );
  }
}

class AccountPage extends StatelessWidget {
  const AccountPage({super.key, required this.state});
  final AppState state;

  @override
  Widget build(BuildContext context) {
    final s = state;
    final a = s.account;
    return ListView(
      padding: const EdgeInsets.all(16),
      children: [
        Row(
          children: [
            Image.asset('assets/f-launcher.png', width: 48, height: 48),
            const SizedBox(width: 12),
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(
                    a?.name ?? 'F-Launcher',
                    style: Theme.of(context).textTheme.titleLarge,
                  ),
                  if (a != null)
                    Text('${s.t('role')}: ${s.t('role.${a.role}')}'),
                ],
              ),
            ),
          ],
        ),
        const SizedBox(height: 16),
        if (s.signedIn)
          OutlinedButton.icon(
            key: const Key('sign-out'),
            onPressed: s.signOut,
            icon: const Icon(Icons.logout),
            label: Text(s.t('sign_out')),
          )
        else
          FilledButton.icon(
            key: const Key('sign-in-account'),
            onPressed: s.signIn,
            icon: const Icon(Icons.login),
            label: Text(s.t('sign_in')),
          ),
        const Divider(height: 32),
        Text(s.t('language'), style: Theme.of(context).textTheme.titleSmall),
        Wrap(
          spacing: 6,
          children: [
            for (final l in Lang.values)
              ChoiceChip(
                key: Key('lang-${l.code}'),
                label: Text(l.label),
                selected: s.lang == l,
                onSelected: (_) => s.lang = l,
              ),
          ],
        ),
        const Divider(height: 32),
        Text(s.t('on_web'), style: Theme.of(context).textTheme.titleSmall),
        for (final (key, path) in [
          ('profile_web', ''),
          ('projects_web', 'projects'),
          ('orders_web', 'orders'),
        ])
          ListTile(
            contentPadding: EdgeInsets.zero,
            leading: const Icon(Icons.open_in_new),
            title: Text(s.t(key)),
            onTap: () => s.openPage(path),
          ),
        const SizedBox(height: 8),
        Text(
          s.t('version', {'v': '1.10.0'}),
          style: Theme.of(context).textTheme.bodySmall,
        ),
      ],
    );
  }
}
