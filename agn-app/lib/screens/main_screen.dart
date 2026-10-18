/// AGN main screen — rikkahub-style chat UI.
///
/// Design reference: rikkahub ChatInput.kt
/// - Input: single rounded Surface container with border, no separate bubble
/// - TextField: borderless, transparent, inside the container
/// - Bottom bar: model picker + more (left), send button (right)
/// - Send: 30dp circle, primary when text, grey when empty, error when loading
/// - Background: OLED black

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
  GenStatus? _genStatus;
  String _pendingText = '';
  bool _backendOnline = false;
  bool _sending = false;
  dynamic _ws;
  bool _wsConnected = false;
  final _inputCtrl = TextEditingController();
  final _scrollCtrl = ScrollController();
  final _drawerKey = GlobalKey<ScaffoldState>();

  @override
  void initState() {
    super.initState();
    _inputCtrl.addListener(() => setState(() {}));
    _initData();
  }

  @override
  void dispose() {
    _inputCtrl.dispose();
    _scrollCtrl.dispose();
    _ws?.sink?.close();
    super.dispose();
  }

  Future<void> _initData() async {
    await _checkBackend();
    if (_backendOnline) {
      await _loadData();
      _connectWs();
    }
  }

  Future<void> _checkBackend() async {
    final ok = await widget.api.ping();
    if (mounted) setState(() => _backendOnline = ok);
  }

  Future<void> _loadData() async {
    try {
      final convs = await widget.api.getConversations();
      final models = await widget.api.getModels();
      if (mounted) {
        setState(() {
          _conversations = convs;
          _models = models;
        });
        if (convs.isNotEmpty) await _selectConv(convs.first.id);
      }
    } catch (e) {
      debugPrint('Load error: $e');
    }
  }

  void _connectWs() {
    try {
      _ws?.sink?.close();
      _ws = widget.api.connectWs();
      setState(() => _wsConnected = true);
      _ws.stream.listen(
        (data) {
          try {
            final msg = jsonDecode(data.toString());
            _handleWsMessage(msg);
          } catch (e) {
            debugPrint('WS parse error: $e');
          }
        },
        onError: (e) {
          debugPrint('WS error: $e');
          if (mounted) setState(() => _wsConnected = false);
          Future.delayed(const Duration(seconds: 3), _connectWs);
        },
        onDone: () {
          if (mounted) setState(() => _wsConnected = false);
          Future.delayed(const Duration(seconds: 3), _connectWs);
        },
      );
    } catch (e) {
      debugPrint('WS connect error: $e');
      if (mounted) setState(() => _wsConnected = false);
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
          _sending = false;
          if (_pendingText.isNotEmpty && _currentConvId != null) {
            _selectConv(_currentConvId!);
            _pendingText = '';
          }
        });
        break;
      case 'error':
        setState(() {
          _genStatus = GenStatus(state: 'error', message: data['message']);
          _sending = false;
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
      if (Navigator.canPop(context)) Navigator.pop(context);
    } catch (e) {
      _snack('创建失败: $e');
    }
  }

  void _sendMessage() async {
    final text = _inputCtrl.text.trim();
    if (text.isEmpty) return;

    if (!_backendOnline) {
      _snack('后端未连接，请先启动 agn serve');
      return;
    }

    if (_currentConvId == null) {
      try {
        await _newChat();
      } catch (e) {
        _snack('创建对话失败: $e');
        return;
      }
    }
    if (_currentConvId == null || _ws == null) return;

    _inputCtrl.clear();
    setState(() {
      _sending = true;
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

  bool get _isInputEmpty => _inputCtrl.text.trim().isEmpty;

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      key: _drawerKey,
      drawer: _buildDrawer(),
      backgroundColor: Colors.black,
      body: SafeArea(
        bottom: false,
        child: Column(
          children: [
            _buildTopBar(),
            if (!_backendOnline) _buildOfflineBanner(),
            Expanded(child: _buildMessageList()),
            _buildChatInput(),
          ],
        ),
      ),
    );
  }

  // === Top Bar ===
  Widget _buildTopBar() {
    final title = _currentConv?.title ?? 'AGN Studios';
    return Container(
      height: 52,
      padding: const EdgeInsets.symmetric(horizontal: 4),
      child: Row(
        children: [
          IconButton(
            icon: const Icon(Icons.menu, size: 22),
            onPressed: () => _drawerKey.currentState?.openDrawer(),
            tooltip: '对话列表',
          ),
          Expanded(
            child: Text(
              title,
              maxLines: 1,
              overflow: TextOverflow.ellipsis,
              style: const TextStyle(fontSize: 16, fontWeight: FontWeight.w500),
            ),
          ),
          Container(
            width: 7, height: 7,
            margin: const EdgeInsets.only(right: 8),
            decoration: BoxDecoration(
              color: _wsConnected ? StatusColors.green : StatusColors.red,
              shape: BoxShape.circle,
            ),
          ),
          IconButton(
            icon: const Icon(Icons.add, size: 22),
            onPressed: _newChat,
            tooltip: '新对话',
          ),
          IconButton(
            icon: const Icon(Icons.settings_outlined, size: 20),
            onPressed: () => Navigator.push(context,
              MaterialPageRoute(builder: (_) => SettingsScreen(api: widget.api))),
          ),
        ],
      ),
    );
  }

  Widget _buildOfflineBanner() {
    return Container(
      padding: const EdgeInsets.symmetric(vertical: 6, horizontal: 16),
      color: StatusColors.red.withOpacity(0.1),
      child: Row(
        mainAxisAlignment: MainAxisAlignment.center,
        children: [
          Icon(Icons.cloud_off, size: 14, color: StatusColors.red),
          const SizedBox(width: 8),
          Text('后端未连接', style: TextStyle(fontSize: 12, color: StatusColors.red)),
          const SizedBox(width: 8),
          TextButton(
            onPressed: _initData,
            style: TextButton.styleFrom(
              minimumSize: Size.zero,
              padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 2),
              tapTargetSize: MaterialTapTargetSize.shrinkWrap,
            ),
            child: const Text('重试', style: TextStyle(fontSize: 12)),
          ),
        ],
      ),
    );
  }

  // === Message List ===
  Widget _buildMessageList() {
    if (_genStatus != null && _genStatus!.state == 'generating') {
      final s = _genStatus!;
      return Column(children: [
        Container(
          padding: const EdgeInsets.symmetric(vertical: 4, horizontal: 16),
          child: Row(
            mainAxisAlignment: MainAxisAlignment.center,
            children: [
              const SizedBox(width: 12, height: 12,
                child: CircularProgressIndicator(strokeWidth: 2)),
              const SizedBox(width: 8),
              Text(s.message ?? '正在生成...', style: const TextStyle(fontSize: 12)),
            ],
          ),
        ),
        Expanded(child: _buildMessageListView()),
      ]);
    }
    return _buildMessageListView();
  }

  Widget _buildMessageListView() {
    if (_currentConv == null) {
      return Center(
        child: Column(
          mainAxisAlignment: MainAxisAlignment.center,
          children: [
            Icon(Icons.chat_bubble_outline, size: 56, color: Colors.grey[700]),
            const SizedBox(height: 12),
            Text('选择一个对话或创建新对话',
              style: TextStyle(fontSize: 13, color: Colors.grey[600])),
          ],
        ),
      );
    }

    final msgs = _currentConv!.messages;
    final hasPending = _pendingText.isNotEmpty;
    return ListView.builder(
      controller: _scrollCtrl,
      padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 4),
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
        margin: const EdgeInsets.only(bottom: 10),
        child: Row(
          mainAxisAlignment: isUser ? MainAxisAlignment.end : MainAxisAlignment.start,
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            if (!isUser) _buildAvatar(false),
            if (!isUser) const SizedBox(width: 8),
            Flexible(
              child: Container(
                padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 10),
                decoration: BoxDecoration(
                  color: isUser
                    ? const Color(0xFFE94560).withOpacity(0.12)
                    : const Color(0xFF1E1E1E),
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
                      style: const TextStyle(fontSize: 14, height: 1.5, color: Color(0xFFE0E0E0)),
                    ),
                    if (msg.toolCalls != null) ...[
                      for (final tc in msg.toolCalls!)
                        Container(
                          margin: const EdgeInsets.only(top: 8),
                          padding: const EdgeInsets.all(8),
                          decoration: BoxDecoration(
                            color: Colors.black54,
                            borderRadius: BorderRadius.circular(8),
                            border: Border.all(color: const Color(0xFF333333)),
                          ),
                          child: Row(children: [
                            const Icon(Icons.build, size: 12, color: Color(0xFFE94560)),
                            const SizedBox(width: 4),
                            Text(tc.name, style: const TextStyle(
                              fontSize: 11, fontWeight: FontWeight.w600, color: Color(0xFFE94560))),
                          ]),
                        ),
                    ],
                    if (msg.usage != null) ...[
                      const SizedBox(height: 4),
                      Wrap(spacing: 8, children: [
                        _statChip('↑${msg.usage!.promptTokens}'),
                        _statChip('↓${msg.usage!.completionTokens}'),
                      ]),
                    ],
                  ],
                ),
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
    return Text(text, style: TextStyle(fontSize: 10, color: Colors.grey[600]));
  }

  Widget _buildAvatar(bool isUser) {
    return Container(
      width: 28, height: 28,
      decoration: BoxDecoration(
        color: isUser
          ? const Color(0xFFE94560).withOpacity(0.2)
          : const Color(0xFF533483).withOpacity(0.3),
        shape: BoxShape.circle,
      ),
      child: Icon(
        isUser ? Icons.person : Icons.smart_toy,
        size: 16,
        color: isUser ? const Color(0xFFE94560) : Colors.grey[400],
      ),
    );
  }

  Widget _buildPendingItem() {
    return Container(
      margin: const EdgeInsets.only(bottom: 10),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          _buildAvatar(false),
          const SizedBox(width: 8),
          Flexible(
            child: Container(
              padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 10),
              decoration: BoxDecoration(
                color: const Color(0xFF1E1E1E),
                borderRadius: BorderRadius.circular(16).copyWith(
                  topLeft: const Radius.circular(4),
                ),
              ),
              child: _pendingText.isEmpty
                ? const SizedBox(width: 14, height: 14,
                    child: CircularProgressIndicator(strokeWidth: 2))
                : SelectableText(_pendingText,
                    style: const TextStyle(fontSize: 14, height: 1.5, color: Color(0xFFE0E0E0))),
            ),
          ),
        ],
      ),
    );
  }

  void _showMsgMenu(ChatMessage msg) {
    showMenu(
      context: context,
      position: const RelativeRect.fromLTRB(200, 400, 200, 400),
      items: [
        const PopupMenuItem(value: 'copy', child: Text('复制')),
        const PopupMenuItem(value: 'delete', child: Text('删除', style: TextStyle(color: Colors.red))),
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

  // ================================================================
  // Chat Input — rikkahub style
  // Single rounded surface with border, text field inside,
  // bottom row with model picker (left) + send button (right)
  // ================================================================
  Widget _buildChatInput() {
    final cs = Theme.of(context).colorScheme;
    final showStop = _sending && _isInputEmpty;

    return SafeArea(
      top: false,
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          // Container with border + rounded corners
          Container(
            margin: const EdgeInsets.symmetric(horizontal: 8, vertical: 6),
            decoration: BoxDecoration(
              color: const Color(0xFF1A1A1A),
              borderRadius: BorderRadius.circular(24),
              border: Border.all(
                color: const Color(0xFF333333),
                width: 0.5,
              ),
            ),
            child: Column(
              mainAxisSize: MainAxisSize.min,
              children: [
                // TextField
                TextField(
                  controller: _inputCtrl,
                  maxLines: 5,
                  minLines: 1,
                  textInputAction: TextInputAction.send,
                  style: const TextStyle(fontSize: 15, color: Color(0xFFE0E0E0)),
                  decoration: const InputDecoration(
                    hintText: '输入消息...',
                    hintStyle: TextStyle(color: Color(0xFF666666)),
                    contentPadding: EdgeInsets.symmetric(horizontal: 16, vertical: 12),
                    border: InputBorder.none,
                    enabledBorder: InputBorder.none,
                    focusedBorder: InputBorder.none,
                    filled: false,
                  ),
                  onSubmitted: (_) => _sendMessage(),
                ),
                // Bottom action bar
                Padding(
                  padding: const EdgeInsets.only(left: 8, right: 4, bottom: 4),
                  child: Row(
                    children: [
                      // Left: model picker (if models available)
                      if (_models.isNotEmpty)
                        _buildModelPicker(cs),
                      if (_models.isNotEmpty) const SizedBox(width: 4),
                      // More button
                      _buildIconButton(
                        icon: Icons.more_horiz,
                        onTap: () => _snack('更多功能 (待实现)'),
                        cs: cs,
                      ),
                      const Spacer(),
                      // Send / Stop button
                      _buildSendButton(showStop, cs),
                    ],
                  ),
                ),
              ],
            ),
          ),
        ],
      ),
    );
  }

  Widget _buildModelPicker(ColorScheme cs) {
    return PopupMenuButton<String>(
      child: Container(
        height: 30,
        padding: const EdgeInsets.symmetric(horizontal: 8),
        decoration: BoxDecoration(
          shape: BoxShape.circle,
          color: Colors.transparent,
        ),
        child: Row(
          mainAxisSize: MainAxisSize.min,
          children: [
            Icon(Icons.smart_toy, size: 14, color: Colors.grey[500]),
            const SizedBox(width: 4),
            Text(
              _selectedModel ?? '模型',
              style: TextStyle(fontSize: 11, color: Colors.grey[500]),
            ),
          ],
        ),
      ),
      onSelected: (v) => setState(() => _selectedModel = v),
      itemBuilder: (_) => _models.map((m) => PopupMenuItem(
        value: m.id,
        child: Text('${m.id} (${m.provider})', style: const TextStyle(fontSize: 13)),
      )).toList(),
    );
  }

  Widget _buildIconButton({
    required IconData icon,
    required VoidCallback onTap,
    required ColorScheme cs,
  }) {
    return InkWell(
      onTap: onTap,
      borderRadius: BorderRadius.circular(15),
      child: Container(
        width: 30, height: 30,
        decoration: const BoxDecoration(shape: BoxShape.circle, color: Colors.transparent),
        child: Icon(icon, size: 18, color: Colors.grey[500]),
      ),
    );
  }

  Widget _buildSendButton(bool showStop, ColorScheme cs) {
    final empty = _isInputEmpty && !showStop;
    final containerColor = showStop
      ? cs.errorContainer
      : empty
        ? const Color(0xFF242424)
        : cs.primary;
    final contentColor = showStop
      ? cs.onErrorContainer
      : empty
        ? const Color(0xFF666666)
        : cs.onPrimary;

    return GestureDetector(
      onTap: (showStop || !empty) ? _sendMessage : null,
      child: Container(
        width: 30, height: 30,
        decoration: BoxDecoration(
          shape: BoxShape.circle,
          color: containerColor,
        ),
        child: Icon(
          showStop ? Icons.stop : Icons.arrow_upward,
          size: 18,
          color: contentColor,
        ),
      ),
    );
  }

  // === Drawer ===
  Widget _buildDrawer() {
    return Drawer(
      backgroundColor: Colors.black,
      child: SafeArea(
        child: Column(
          children: [
            Container(
              padding: const EdgeInsets.all(16),
              child: Row(
                children: [
                  ShaderMask(
                    shaderCallback: (b) => const LinearGradient(
                      colors: [Color(0xFFE94560), Color(0xFF533483)],
                    ).createShader(b),
                    child: const Text('⚡ AGN', style: TextStyle(
                      fontSize: 18, fontWeight: FontWeight.bold, color: Colors.white)),
                  ),
                  const Spacer(),
                  IconButton(
                    icon: const Icon(Icons.key_outlined, size: 20),
                    onPressed: () => Navigator.push(context,
                      MaterialPageRoute(builder: (_) => KeyStatusScreen(api: widget.api))),
                  ),
                ],
              ),
            ),
            Padding(
              padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 4),
              child: FilledButton.icon(
                onPressed: _newChat,
                icon: const Icon(Icons.add, size: 18),
                label: const Text('新对话'),
                style: FilledButton.styleFrom(
                  minimumSize: const Size(double.infinity, 40),
                  shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(12)),
                ),
              ),
            ),
            Expanded(
              child: _conversations.isEmpty
                ? Center(child: Text('没有对话记录',
                    style: TextStyle(color: Colors.grey[600], fontSize: 13)))
                : ListView.builder(
                    padding: const EdgeInsets.symmetric(vertical: 4),
                    itemCount: _conversations.length,
                    itemBuilder: (ctx, i) {
                      final c = _conversations[i];
                      final isActive = c.id == _currentConvId;
                      return ListTile(
                        dense: true,
                        selected: isActive,
                        selectedTileColor: const Color(0xFF1A1A1A),
                        title: Text(c.title, maxLines: 1,
                          overflow: TextOverflow.ellipsis,
                          style: TextStyle(fontSize: 13, color: isActive ? Colors.white : Colors.grey[400])),
                        onTap: () {
                          _selectConv(c.id);
                          Navigator.pop(context);
                        },
                        onLongPress: () => _showConvMenu(c),
                      );
                    },
                  ),
            ),
          ],
        ),
      ),
    );
  }

  void _showConvMenu(Conversation c) {
    showModalBottomSheet(
      context: context,
      backgroundColor: const Color(0xFF1A1A1A),
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