/// AGN settings screen — simplified.
import 'package:flutter/material.dart';
import '../services/agn_api.dart';
import '../models/agn_models.dart';

class SettingsScreen extends StatefulWidget {
  final AgnApi api;
  const SettingsScreen({super.key, required this.api});

  @override
  State<SettingsScreen> createState() => _SettingsScreenState();
}

class _SettingsScreenState extends State<SettingsScreen> {
  Map<String, dynamic>? _config;

  @override
  void initState() {
    super.initState();
    _load();
  }

  Future<void> _load() async {
    try {
      final cfg = await widget.api.getConfig();
      setState(() => _config = cfg);
    } catch (e) {
      debugPrint('Settings load: $e');
    }
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(title: Text('设置')),
      body: _config == null
          ? Center(child: CircularProgressIndicator())
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