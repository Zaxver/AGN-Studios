/// AGN key status screen — shows gateway API key pool status.

import 'package:flutter/material.dart';
import '../services/agn_api.dart';
import '../services/agn_theme.dart';
import '../models/agn_models.dart';

class KeyStatusScreen extends StatefulWidget {
  final AgnApi api;
  const KeyStatusScreen({super.key, required this.api});

  @override
  State<KeyStatusScreen> createState() => _KeyStatusScreenState();
}

class _KeyStatusScreenState extends State<KeyStatusScreen> {
  List<KeyStatusInfo> _statuses = [];
  bool _loading = true;

  @override
  void initState() {
    super.initState();
    _load();
  }

  Future<void> _load() async {
    try {
      final statuses = await widget.api.getKeyStatus();
      setState(() {
        _statuses = statuses;
        _loading = false;
      });
    } catch (e) {
      setState(() => _loading = false);
    }
  }

  Color _statusColor(String status) {
    switch (status) {
      case 'available':
        return StatusColors.green;
      case 'cooling':
      case 'networkerror':
        return StatusColors.yellow;
      case 'invalid':
        return StatusColors.red;
      default:
        return Colors.grey;
    }
  }

  String _statusLabel(String status) {
    switch (status) {
      case 'available':
        return '可用';
      case 'cooling':
        return '冷却中';
      case 'networkerror':
        return '网络异常';
      case 'invalid':
        return '失效 (401)';
      default:
        return '未使用';
    }
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(
        title: Text('API Key 状态'),
        actions: [
          IconButton(
            icon: Icon(Icons.refresh),
            onPressed: () {
              setState(() => _loading = true);
              _load();
            },
          ),
        ],
      ),
      body: _loading
          ? Center(child: CircularProgressIndicator())
          : _statuses.isEmpty
              ? Center(child: Text('暂无Key'))
              : ListView.builder(
                  padding: const EdgeInsets.all(16),
                  itemCount: _statuses.length,
                  itemBuilder: (ctx, i) {
                    final s = _statuses[i];
                    return Card(
                      child: ListTile(
                        leading: Container(
                          width: 12,
                          height: 12,
                          decoration: BoxDecoration(
                            color: _statusColor(s.status),
                            shape: BoxShape.circle,
                          ),
                        ),
                        title: Text(s.providerName),
                        subtitle: Column(
                          crossAxisAlignment: CrossAxisAlignment.start,
                          children: [
                            Text(s.keyLabel.isNotEmpty
                                ? s.keyLabel
                                : s.keyId.substring(0, s.keyId.length.clamp(0, 8))),
                            Text(_statusLabel(s.status),
                                style: TextStyle(
                                    color: _statusColor(s.status), fontSize: 12)),
                            if (s.cooldownRemaining != null)
                              Text('${s.cooldownRemaining}s 后恢复',
                                  style: TextStyle(fontSize: 11)),
                            if (s.lastError != null)
                              Text(s.lastError!.substring(0, s.lastError!.length.clamp(0, 60)),
                                  style: TextStyle(fontSize: 11, color: StatusColors.red)),
                          ],
                        ),
                      ),
                    );
                  },
                ),
    );
  }
}