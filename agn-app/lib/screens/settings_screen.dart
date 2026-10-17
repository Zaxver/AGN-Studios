/// AGN settings screen — simplified with error handling.
import 'package:flutter/material.dart';
import '../services/agn_api.dart';

class SettingsScreen extends StatefulWidget {
  final AgnApi api;
  const SettingsScreen({super.key, required this.api});

  @override
  State<SettingsScreen> createState() => _SettingsScreenState();
}

class _SettingsScreenState extends State<SettingsScreen> {
  Map<String, dynamic>? _config;
  String? _error;
  bool _loading = true;

  @override
  void initState() {
    super.initState();
    _load();
  }

  Future<void> _load() async {
    setState(() {
      _loading = true;
      _error = null;
    });
    try {
      final cfg = await widget.api.getConfig();
      if (mounted) setState(() {
        _config = cfg;
        _loading = false;
      });
    } catch (e) {
      if (mounted) setState(() {
        _error = e.toString();
        _loading = false;
      });
    }
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(title: const Text('设置')),
      body: _loading
        ? const Center(child: CircularProgressIndicator())
        : _error != null
          ? Center(
              child: Column(
                mainAxisAlignment: MainAxisAlignment.center,
                children: [
                  const Icon(Icons.cloud_off, size: 48),
                  const SizedBox(height: 16),
                  Text('加载失败', style: Theme.of(context).textTheme.titleMedium),
                  const SizedBox(height: 8),
                  Text(_error!, style: TextStyle(fontSize: 12, color: Theme.of(context).hintColor)),
                  const SizedBox(height: 16),
                  ElevatedButton(onPressed: _load, child: const Text('重试')),
                ],
              ),
            )
          : ListView(padding: const EdgeInsets.all(16), children: [
              Card(child: Padding(padding: const EdgeInsets.all(16), child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text('通用', style: Theme.of(context).textTheme.titleSmall),
                  const SizedBox(height: 12),
                  Text('主题: ${_config!['general']?['theme'] ?? 'dark'}'),
                  Text('权限: ${_config!['general']?['permission'] ?? 'fullaccess'}'),
                ],
              ))),
              const SizedBox(height: 16),
              Card(child: Padding(padding: const EdgeInsets.all(16), child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text('Agent', style: Theme.of(context).textTheme.titleSmall),
                  const SizedBox(height: 12),
                  Text('模型: ${_config!['agent']?['selected_model'] ?? '(未设置)'}'),
                  Text('上下文: ${_config!['agent']?['global_context_length'] ?? 32768}'),
                  Text('自动压缩: ${_config!['agent']?['auto_compress']?['enabled'] ?? true}'),
                ],
              ))),
              const SizedBox(height: 16),
              Card(child: Padding(padding: const EdgeInsets.all(16), child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text('网关', style: Theme.of(context).textTheme.titleSmall),
                  const SizedBox(height: 12),
                  Text('提供商数: ${(_config!['gateway']?['providers'] as List?)?.length ?? 0}'),
                ],
              ))),
              const SizedBox(height: 16),
              Card(child: Padding(padding: const EdgeInsets.all(16), child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text('服务端', style: Theme.of(context).textTheme.titleSmall),
                  const SizedBox(height: 12),
                  Text('API端口: ${_config!['server']?['api_port'] ?? 11828}'),
                  Text('WebUI端口: ${_config!['server']?['webui_port'] ?? 11829}'),
                  Text('本地启动: ${_config!['server']?['start_local'] ?? true}'),
                ],
              ))),
            ]),
    );
  }
}