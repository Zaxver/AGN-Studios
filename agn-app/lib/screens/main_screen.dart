/// AGN main screen — rikkahub-style chat UI.
///
/// Key fixes:
/// - SafeArea to avoid status bar overlap
/// - Drawer for mobile sidebar
/// - Auto-create conversation on first message
/// - Proper WebSocket send with conversation_id
/// - imopathic / keyboard handling

import 'dart:convert';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import '../models/agn_models.dart';
import '../services/agn_api.dart';
import '../services/agn_theme.dart';
import 'settings_screen.dart';
import 'key_status_screen.dart';

class MainScreen extends StatefulWidget {
  final AgnApi api;
  const MainScreen({super.key, required this.api});

  @override
  State<MainScreen> createState() => _MainScreenState();
}

class _MainScreenState extends State<MainScreen> {
  List<Conversation> _conversations = [];
  String? _currentConvId;
  Conversation? _currentConv;
  List<ModelInfo> _models = [];
  String? _selectedModel;
  int _thinkingDepth = 0;
  dynamic _ws;
  final _inputCtrl = TextEditingController();
  final _scrollCtrl = ScrollController();
  GenStatus? _genStatus;
  String _pendingText = '';
  final _drawerKey = GlobalKey<ScaffoldState>();

  @override
  void initState() {
    super.initState();
    _loadData();
    _connectWs();
  }

  Future<void> _loadData() async {
    try {
      final convs = await widget.api.getConversations();
      final models = await widget.api.getModels();
      setState(() {
        _conversations = convs;
        _models = models;
      });
      if (convs.isNotEmpty) {
        await _selectConv(convs.first.id);
      }
    } catch (e) {
      debugPrint('Load error: $e');
    }
  }

  void _connectWs() {
    try {
      _ws = widget.api.connectWs();
      _ws.stream.listen((data) {
        final msg = jsonDecode(data.toString());
        _handleWsMessage(msg);
      }, onDone: () {
        Future.delayed(const Duration(seconds: 3), _connectWs);
      });
    } catch (e) {
      debugPrint('WS error: $e');
      Future.delayed(const Duration(seconds: 3), _connectWs);
    }
  }

  void _handleWsMessage(Map<String, dynamic> data) {
    switch (data['type']) {
      case 'status':
        setState(() => _genStatus = GenStatus.fromJson(data));
        break;
      case 'text_chunk':
        setState(() => _pendingText += data['content'] ?? '');
        _scrollToBottom();
        break;
      case 'done':
        setState(() {
          _genStatus = null;
          if (_pendingText.isNotEmpty && _currentConvId != null) {
            _selectConv(_currentConvId!);
            _pendingText = '';
          }
        });
        break;
      case 'error':
        setState(() {
          _genStatus = GenStatus(state: 'error', message: data['message']);
        });
        break;
    }
  }

  Future<void> _selectConv(String id) async {
    try {
      final conv = await widget.api.getConversation(id);
      setState(() {
        _currentConvId = id;
        _currentConv = conv;
        _pendingText = '';
      });
      _scrollToBottom();
    } catch (e) {
      debugPrint('Select error: $e');
    }
  }

  Future<void> _newChat() async {
    try {
      final conv = await widget.api.createConversation('新对话');
      setState(() {
        _conversations.insert(0, conv);
        _currentConvId = conv.id;
        _currentConv = conv;
        _pendingText = '';
      });
      if (Navigator.canPop(context)) Navigator.pop(context); // close drawer
    } catch (e) {
      _snack('Failed: $e');
    }
  }

  Future<void> _ensureConversation() async {
    if (_currentConvId == null) {
      await _newChat();
    }
  }

  void _sendMessage() async {
    final text = _inputCtrl.text.trim();
    if (text.isEmpty) return;
    await _ensureConversation();
    if (_currentConvId == null || _ws == null) return;

    _inputCtrl.clear();
    setState(() {
      if (_currentConv != null) {
        _currentConv!.messages.add(ChatMessage(
          id: 'p${DateTime.now().millisecondsSinceEpoch}',
          role: 'user',
          content: text,
        ));
      }
      _genStatus = GenStatus(state: 'generating', message: '正在生成回复...');
    });
    _ws.sink.add(jsonEncode({
      'type': 'chat',
      'conversation_id': _currentConvId,
      'content': text,
    }));
    _scrollToBottom();
  }

  void _scrollToBottom() {
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (_scrollCtrl.hasClients) {
        _scrollCtrl.animateTo(
          _scrollCtrl.position.maxScrollExtent,
          duration: const Duration(milliseconds: 200),
          curve: Curves.easeOut,
        );
      }
    });
  }

  void _snack(String msg) {
    ScaffoldMessenger.of(context).showSnackBar(SnackBar(content: Text(msg)));
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      key: _drawerKey,
      drawer: _buildDrawer(),
      body: SafeArea(
        bottom: false,
        child: Column(
          children: [
            _buildTopBar(),
            if (_genStatus != null) _buildGenStatus(),
            Expanded(child: _buildMessageList()),
            _buildInputArea(),
          ],
        ),
      ),
    );
  }

  // === Top Bar ===
  Widget _buildTopBar() {
    final title = _currentConv?.title ?? 'AGN Studios';
    final isWide = MediaQuery.of(context).size.width > 768;

    return Container(
      height: 56,
      padding: const EdgeInsets.symmetric(horizontal: 4),
      child: Row(
        children: [
          if (!isWide)
            IconButton(
              icon: const Icon(Icons.menu),
              onPressed: () => _drawerKey.currentState?.openDrawer(),
            ),
          Expanded(
            child: GestureDetector(
              onTap: () {},
              child: Column(
                mainAxisAlignment: MainAxisAlignment.center,
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(
                    title,
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                    style: const TextStyle(fontSize: 15, fontWeight: FontWeight.w500),
                  ),
                  if (_models.isNotEmpty && _selectedModel != null)
                    Text(
                      _models.firstWhere((m) => m.id == _selectedModel,
                          orElse: () => _models.first).id,
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: TextStyle(
                        fontSize: 10,
                        color: Theme.of(context).hintColor,
                      ),
                    ),
                ],
              ),
            ),
          ),
          // Model picker
          PopupMenuButton<String>(
            child: Container(
              padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 6),
              child: Row(
                mainAxisSize: MainAxisSize.min,
                children: [
                  Icon(Icons.smart_toy, size: 16, color: Theme.of(context).hintColor),
                  const SizedBox(width: 4),
                  Text(
                    _selectedModel ?? '模型',
                    style: TextStyle(fontSize: 12, color: Theme.of(context).hintColor),
                  ),
                ],
              ),
            ),
            onSelected: (v) => setState(() => _selectedModel = v),
            itemBuilder: (_) => _models.map((m) => PopupMenuItem(
              value: m.id,
              child: Text('${m.id} (${m.provider})'),
            )).toList(),
          ),
          // New chat
          IconButton(
            icon: const Icon(Icons.add_comment_outlined),
            onPressed: _newChat,
            tooltip: '新对话',
          ),
          // Settings
          IconButton(
            icon: const Icon(Icons.settings_outlined),
            onPressed: () => Navigator.push(context,
              MaterialPageRoute(builder: (_) => SettingsScreen(api: widget.api))),
          ),
        ],
      ),
    );
  }

  // === Generation Status ===
  Widget _buildGenStatus() {
    final s = _genStatus!;
    Color? light;
    String text;
    bool spin = true;
    switch (s.state) {
      case 'generating': text = s.message ?? '正在生成回复...'; break;
      case 'retrying': text = '重试中...[${s.current}/${s.total}]'; break;
      case 'network': text = '网络波动...[${s.current}/${s.total}]'; light = StatusColors.yellow; break;
      case 'waiting_user': text = '等待用户回复...'; light = StatusColors.yellow; spin = false; break;
      case 'error': text = s.message ?? '生成失败'; light = StatusColors.red; spin = false; break;
      default: text = ''; spin = false;
    }
    return Container(
      padding: const EdgeInsets.symmetric(vertical: 6, horizontal: 16),
      child: Row(
        mainAxisAlignment: MainAxisAlignment.center,
        children: [
          if (spin) ...[
            const SizedBox(
              width: 14, height: 14,
              child: CircularProgressIndicator(strokeWidth: 2),
            ),
            const SizedBox(width: 8),
          ],
          if (light != null) ...[
            Container(width: 8, height: 8,
              decoration: BoxDecoration(color: light, shape: BoxShape.circle)),
            const SizedBox(width: 8),
          ],
          Text(text, style: const TextStyle(fontSize: 13)),
        ],
      ),
    );
  }

  // === Message List ===
  Widget _buildMessageList() {
    if (_currentConv == null) {
      return Center(
        child: Column(
          mainAxisAlignment: MainAxisAlignment.center,
          children: [
            Icon(Icons.chat_bubble_outline, size: 64, color: Theme.of(context).hintColor),
            const SizedBox(height: 16),
            Text('选择一个对话或创建新对话', style: TextStyle(color: Theme.of(context).hintColor)),
          ],
        ),
      );
    }

    final msgs = _currentConv!.messages;
    final hasPending = _pendingText.isNotEmpty;
    return ListView.builder(
      controller: _scrollCtrl,
      padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 8),
      itemCount: msgs.length + (hasPending ? 1 : 0),
      itemBuilder: (ctx, i) {
        if (i < msgs.length) return _buildMessageItem(msgs[i]);
        return _buildPendingItem();
      },
    );
  }

  Widget _buildMessageItem(ChatMessage msg) {
    final isUser = msg.role == 'user';
    return GestureDetector(
      onLongPress: () => _showMsgMenu(msg),
      child: Container(
        margin: const EdgeInsets.only(bottom: 12),
        child: Row(
          mainAxisAlignment: isUser ? MainAxisAlignment.end : MainAxisAlignment.start,
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            if (!isUser) _buildAvatar(false),
            if (!isUser) const SizedBox(width: 8),
            Flexible(
              child: Column(
                crossAxisAlignment: isUser ? CrossAxisAlignment.end : CrossAxisAlignment.start,
                children: [
                  Container(
                    padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 10),
                    decoration: BoxDecoration(
                      color: isUser
                        ? Theme.of(context).colorScheme.primary.withOpacity(0.15)
                        : Theme.of(context).colorScheme.surfaceContainerHighest,
                      borderRadius: BorderRadius.circular(16).copyWith(
                        topLeft: isUser ? const Radius.circular(16) : const Radius.circular(4),
                        topRight: isUser ? const Radius.circular(4) : const Radius.circular(16),
                      ),
                    ),
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        SelectableText(
                          msg.content,
                          style: TextStyle(
                            fontSize: 14,
                            height: 1.5,
                            color: Theme.of(context).colorScheme.onSurface,
                          ),
                        ),
                        // Tool calls
                        if (msg.toolCalls != null) ...[
                          for (final tc in msg.toolCalls!)
                            Container(
                              margin: const EdgeInsets.only(top: 8),
                              padding: const EdgeInsets.all(10),
                              decoration: BoxDecoration(
                                color: Theme.of(context).scaffoldBackgroundColor,
                                borderRadius: BorderRadius.circular(8),
                                border: Border.all(color: Theme.of(context).dividerColor),
                              ),
                              child: Column(
                                crossAxisAlignment: CrossAxisAlignment.start,
                                children: [
                                  Row(
                                    children: [
                                      Icon(Icons.build, size: 14, color: Theme.of(context).colorScheme.primary),
                                      const SizedBox(width: 4),
                                      Text(tc.name, style: TextStyle(
                                        fontSize: 12, fontWeight: FontWeight.w600,
                                        color: Theme.of(context).colorScheme.primary,
                                      )),
                                    ],
                                  ),
                                  const SizedBox(height: 4),
                                  Text(
                                    const JsonEncoder.withIndent('  ').convert(tc.arguments),
                                    style: TextStyle(
                                      fontSize: 11,
                                      color: Theme.of(context).hintColor,
                                      fontFamily: 'monospace',
                                    ),
                                  ),
                                ],
                              ),
                            ),
                        ],
                        // Token stats
                        if (msg.usage != null) ...[
                          const SizedBox(height: 6),
                          Wrap(
                            spacing: 10,
                            children: [
                              _statChip('↑${msg.usage!.promptTokens}'),
                              _statChip('↓${msg.usage!.completionTokens}'),
                              if (msg.usage!.tokensPerSecond != null)
                                _statChip('${msg.usage!.tokensPerSecond!.toStringAsFixed(1)} tok/s'),
                            ],
                          ),
                        ],
                      ],
                    ),
                  ),
                ],
              ),
            ),
            if (isUser) const SizedBox(width: 8),
            if (isUser) _buildAvatar(true),
          ],
        ),
      ),
    );
  }

  Widget _statChip(String text) {
    return Text(text, style: TextStyle(
      fontSize: 10, color: Theme.of(context).hintColor));
  }

  Widget _buildAvatar(bool isUser) {
    return Circle(
      size: 32,
      color: isUser
        ? Theme.of(context).colorScheme.primary.withOpacity(0.3)
        : Theme.of(context).colorScheme.secondaryContainer,
      child: Icon(
        isUser ? Icons.person : Icons.smart_toy,
        size: 18,
        color: isUser
          ? Theme.of(context).colorScheme.primary
          : Theme.of(context).colorScheme.onSecondaryContainer,
      ),
    );
  }

  Widget _buildPendingItem() {
    return Container(
      margin: const EdgeInsets.only(bottom: 12),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          _buildAvatar(false),
          const SizedBox(width: 8),
          Flexible(
            child: Container(
              padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 10),
              decoration: BoxDecoration(
                color: Theme.of(context).colorScheme.surfaceContainerHighest,
                borderRadius: BorderRadius.circular(16).copyWith(
                  topLeft: const Radius.circular(4),
                ),
              ),
              child: SelectableText(
                _pendingText,
                style: TextStyle(fontSize: 14, height: 1.5, color: Theme.of(context).colorScheme.onSurface),
              ),
            ),
          ),
        ],
      ),
    );
  }

  void _showMsgMenu(ChatMessage msg) {
    final RenderBox box = context.findRenderObject() as RenderBox;
    final overlay = Overlay.of(context).context.findRenderObject() as RenderBox;
    showMenu(
      context: context,
      position: RelativeRect.fromRect(
        const Offset(200, 400) & const Size(1, 1),
        Offset.zero & overlay.size,
      ),
      items: [
        const PopupMenuItem(value: 'copy', child: Text('📋 复制')),
        const PopupMenuItem(value: 'edit', child: Text('↩️ 编辑并回滚')),
        const PopupMenuItem(value: 'delete', child: Text('🗑️ 删除', style: TextStyle(color: Colors.red))),
      ],
    ).then((action) {
      if (action == 'copy') {
        Clipboard.setData(ClipboardData(text: msg.content));
        _snack('已复制');
      } else if (action == 'delete' && _currentConvId != null) {
        widget.api.deleteMessage(_currentConvId!, msg.id).then((_) => _selectConv(_currentConvId!));
      }
    });
  }

  // === Input Area ===
  Widget _buildInputArea() {
    return SafeArea(
      top: false,
      child: Container(
        padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 6),
        decoration: BoxDecoration(
          color: Theme.of(context).colorScheme.surfaceContainerHighest.withOpacity(0.5),
          border: Border(
            top: BorderSide(color: Theme.of(context).dividerColor, width: 0.5),
          ),
        ),
        child: Row(
          crossAxisAlignment: CrossAxisAlignment.end,
          children: [
            // Plus menu
            PopupMenuButton<String>(
              icon: Container(
                width: 36, height: 36,
                decoration: BoxDecoration(
                  shape: BoxShape.circle,
                  color: Theme.of(context).colorScheme.surface,
                ),
                child: const Icon(Icons.add, size: 20),
              ),
              onSelected: (v) => _snack('$v (待实现)'),
              itemBuilder: (_) => [
                const PopupMenuItem(value: '上传文件', child: Text('📄 上传文件')),
                const PopupMenuItem(value: '上传图片', child: Text('🖼️ 上传图片')),
                const PopupMenuItem(value: '拍照', child: Text('📷 拍照')),
                const PopupMenuItem(value: '绑定工作区', child: Text('📁 绑定工作区')),
              ],
            ),
            const SizedBox(width: 6),
            // Input field
            Expanded(
              child: Container(
                constraints: const BoxConstraints(maxHeight: 120),
                child: TextField(
                  controller: _inputCtrl,
                  maxLines: null,
                  minLines: 1,
                  decoration: InputDecoration(
                    hintText: '输入消息...',
                    isDense: true,
                    contentPadding: const EdgeInsets.symmetric(horizontal: 14, vertical: 10),
                    filled: true,
                    fillColor: Theme.of(context).colorScheme.surface,
                    border: OutlineInputBorder(
                      borderRadius: BorderRadius.circular(24),
                      borderSide: BorderSide.none,
                    ),
                    enabledBorder: OutlineInputBorder(
                      borderRadius: BorderRadius.circular(24),
                      borderSide: BorderSide.none,
                    ),
                    focusedBorder: OutlineInputBorder(
                      borderRadius: BorderRadius.circular(24),
                      borderSide: BorderSide.none,
                    ),
                  ),
                  onSubmitted: (_) => _sendMessage(),
                ),
              ),
            ),
            const SizedBox(width: 6),
            // Send button
            Container(
              width: 40, height: 40,
              decoration: BoxDecoration(
                shape: BoxShape.circle,
                color: Theme.of(context).colorScheme.primary,
              ),
              child: IconButton(
                icon: const Icon(Icons.arrow_upward, size: 20, color: Colors.white),
                onPressed: _sendMessage,
                padding: EdgeInsets.zero,
                constraints: const BoxConstraints(minWidth: 40, minHeight: 40),
              ),
            ),
          ],
        ),
      ),
    );
  }

  // === Drawer (sidebar) ===
  Widget _buildDrawer() {
    final groups = <String, List<Conversation>>{};
    for (final c in _conversations) {
      final g = c.group.isEmpty ? '未分组' : c.group;
      groups.putIfAbsent(g, () => []).add(c);
    }

    return Drawer(
      child: SafeArea(
        child: Column(
          children: [
            // Header
            Container(
              padding: const EdgeInsets.all(16),
              child: Row(
                children: [
                  ShaderMask(
                    shaderCallback: (b) => const LinearGradient(
                      colors: [Color(0xFFE94560), Color(0xFF533483)],
                    ).createShader(b),
                    child: const Text('⚡ AGN', style: TextStyle(
                      fontSize: 20, fontWeight: FontWeight.bold, color: Colors.white)),
                  ),
                  const Spacer(),
                  IconButton(
                    icon: const Icon(Icons.key_outlined),
                    onPressed: () => Navigator.push(context,
                      MaterialPageRoute(builder: (_) => KeyStatusScreen(api: widget.api))),
                    tooltip: 'API Key 状态',
                  ),
                ],
              ),
            ),
            // New chat button
            Padding(
              padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 4),
              child: FilledButton.icon(
                onPressed: _newChat,
                icon: const Icon(Icons.add),
                label: const Text('新对话'),
                style: FilledButton.styleFrom(
                  minimumSize: const Size(double.infinity, 44),
                  shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(12)),
                ),
              ),
            ),
            // Conversation list
            Expanded(
              child: ListView.builder(
                padding: const EdgeInsets.symmetric(vertical: 4),
                itemCount: groups.length,
                itemBuilder: (ctx, gi) {
                  final group = groups.keys.elementAt(gi);
                  final convs = groups[group]!;
                  return Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Padding(
                        padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 8),
                        child: Text(group, style: TextStyle(
                          fontSize: 11, color: Theme.of(context).hintColor,
                          fontWeight: FontWeight.w600)),
                      ),
                      ...convs.map((c) {
                        final isActive = c.id == _currentConvId;
                        return ListTile(
                          dense: true,
                          selected: isActive,
                          selectedTileColor: Theme.of(context).colorScheme.primaryContainer.withOpacity(0.3),
                          leading: _buildConvStatusDot(c),
                          title: Text(c.title, maxLines: 1,
                            overflow: TextOverflow.ellipsis,
                            style: const TextStyle(fontSize: 14)),
                          onTap: () {
                            _selectConv(c.id);
                            Navigator.pop(context);
                          },
                          onLongPress: () => _showConvMenu(c),
                        );
                      }),
                    ],
                  );
                },
              ),
            ),
          ],
        ),
      ),
    );
  }

  Widget _buildConvStatusDot(Conversation conv) {
    final status = _genStatus?.state ?? 'idle';
    Color color = StatusColors.green;
    if (conv.id == _currentConvId) {
      if (status == 'error') color = StatusColors.red;
      else if (status != 'idle') color = StatusColors.yellow;
    }
    return Container(width: 8, height: 8,
      decoration: BoxDecoration(color: color, shape: BoxShape.circle));
  }

  void _showConvMenu(Conversation c) {
    showModalBottomSheet(
      context: context,
      builder: (ctx) => SafeArea(
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            ListTile(
              leading: const Icon(Icons.delete, color: Colors.red),
              title: const Text('删除对话'),
              onTap: () {
                widget.api.deleteConversation(c.id).then((_) {
                  Navigator.pop(ctx);
                  _loadData();
                });
              },
            ),
          ],
        ),
      ),
    );
  }
}

/// Simple circle widget for avatars.
class Circle extends StatelessWidget {
  final double size;
  final Color color;
  final Widget? child;

  const Circle({super.key, required this.size, required this.color, this.child});

  @override
  Widget build(BuildContext context) {
    return Container(
      width: size, height: size,
      decoration: BoxDecoration(
        color: color,
        shape: BoxShape.circle,
      ),
      child: Center(child: child ?? const SizedBox()),
    );
  }
}