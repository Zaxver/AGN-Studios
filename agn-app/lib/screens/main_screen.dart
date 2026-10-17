/// AGN main screen — rikkahub-style chat UI.
///
/// Design reference: rikkahub 2.5.6
/// - TopBar: menu button / title / model picker / new chat / settings
/// - Message list: user right-aligned, AI left-aligned, avatars
/// - Bottom input: rounded text field + send button
/// - Drawer: conversation list with groups
///
/// Key fixes vs previous version:
/// - SafeArea properly wraps both top bar and bottom input
/// - HTTP timeout on all API calls (5s) prevents infinite spinners
/// - Connection status indicator (red dot if backend offline)
/// - Auto-retry WS connection with backoff
/// - Input field is properly focusable

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
  bool _loading = false;
  dynamic _ws;
  bool _wsConnected = false;
  final _inputCtrl = TextEditingController();
  final _scrollCtrl = ScrollController();
  final _drawerKey = GlobalKey<ScaffoldState>();
  final _inputFocus = FocusNode();

  @override
  void initState() {
    super.initState();
    _initData();
  }

  @override
  void dispose() {
    _inputCtrl.dispose();
    _scrollCtrl.dispose();
    _inputFocus.dispose();
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
        if (convs.isNotEmpty) {
          await _selectConv(convs.first.id);
        }
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
          setState(() => _wsConnected = false);
          Future.delayed(const Duration(seconds: 3), _connectWs);
        },
        onDone: () {
          setState(() => _wsConnected = false);
          Future.delayed(const Duration(seconds: 3), _connectWs);
        },
      );
    } catch (e) {
      debugPrint('WS connect error: $e');
      setState(() => _wsConnected = false);
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
            if (!_backendOnline) _buildOfflineBanner(),
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

    return Container(
      height: 56,
      padding: const EdgeInsets.symmetric(horizontal: 4),
      child: Row(
        children: [
          // Drawer toggle
          IconButton(
            icon: const Icon(Icons.menu),
            onPressed: () => _drawerKey.currentState?.openDrawer(),
            tooltip: '对话列表',
          ),
          // Title
          Expanded(
            child: GestureDetector(
              onTap: () {},
              child: Text(
                title,
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style: const TextStyle(fontSize: 16, fontWeight: FontWeight.w500),
              ),
            ),
          ),
          // Connection indicator
          Container(
            width: 8,
            height: 8,
            margin: const EdgeInsets.only(right: 8),
            decoration: BoxDecoration(
              color: _wsConnected ? StatusColors.green : StatusColors.red,
              shape: BoxShape.circle,
            ),
          ),
          // Model picker
          if (_models.isNotEmpty)
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
            icon: const Icon(Icons.add),
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

  // === Offline Banner ===
  Widget _buildOfflineBanner() {
    return Container(
      padding: const EdgeInsets.symmetric(vertical: 6, horizontal: 16),
      color: StatusColors.red.withOpacity(0.15),
      child: Row(
        mainAxisAlignment: MainAxisAlignment.center,
        children: [
          Icon(Icons.cloud_off, size: 16, color: StatusColors.red),
          const SizedBox(width: 8),
          Text('后端未连接 (127.0.0.1:11828)', style: TextStyle(
            fontSize: 12, color: StatusColors.red)),
          const SizedBox(width: 8),
          TextButton(
            onPressed: _initData,
            child: const Text('重试', style: TextStyle(fontSize: 12)),
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
    return Container(
      width: 32, height: 32,
      decoration: BoxDecoration(
        color: isUser
          ? Theme.of(context).colorScheme.primary.withOpacity(0.3)
          : Theme.of(context).colorScheme.secondaryContainer,
        shape: BoxShape.circle,
      ),
      child: Center(
        child: Icon(
          isUser ? Icons.person : Icons.smart_toy,
          size: 18,
          color: isUser
            ? Theme.of(context).colorScheme.primary
            : Theme.of(context).colorScheme.onSecondaryContainer,
        ),
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
              child: _pendingText.isEmpty
                ? const SizedBox(
                    width: 16, height: 16,
                    child: CircularProgressIndicator(strokeWidth: 2),
                  )
                : SelectableText(
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
    showMenu(
      context: context,
      position: const RelativeRect.fromLTRB(200, 400, 200, 400),
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

  // === Input Area — rikkahub style ===
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
                  focusNode: _inputFocus,
                  maxLines: null,
                  minLines: 1,
                  textInputAction: TextInputAction.send,
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
              child: _conversations.isEmpty
                ? Center(child: Text('没有对话记录', style: TextStyle(color: Theme.of(context).hintColor)))
                : ListView.builder(
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