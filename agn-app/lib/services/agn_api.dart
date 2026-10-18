/// AGN API client — uses dart:io HttpClient directly.
/// This bypasses Android's cleartext HTTP restriction.
import 'dart:convert';
import 'dart:io';
import 'package:web_socket_channel/web_socket_channel.dart';
import '../models/agn_models.dart';

class AgnApi {
  String baseUrl;
  late final String wsUrl;
  Duration timeout = const Duration(seconds: 5);
  late final HttpClient _client;

  AgnApi({this.baseUrl = 'http://127.0.0.1:11828'}) {
    wsUrl = baseUrl.replaceFirst('http', 'ws') + '/ws';
    _client = HttpClient();
    _client.badCertificateCallback = (cert, host, port) => true;
  }

  Future<HttpClientResponse> _get(String path) async {
    final req = await _client.getUrl(Uri.parse('$baseUrl$path'));
    return await req.close().timeout(timeout);
  }

  Future<HttpClientResponse> _post(String path, {Map<String, dynamic>? body}) async {
    final req = await _client.postUrl(Uri.parse('$baseUrl$path'));
    if (body != null) {
      req.headers.contentType = ContentType.json;
      req.write(jsonEncode(body));
    }
    return await req.close().timeout(timeout);
  }

  Future<HttpClientResponse> _put(String path, {Map<String, dynamic>? body}) async {
    final req = await _client.putUrl(Uri.parse('$baseUrl$path'));
    if (body != null) {
      req.headers.contentType = ContentType.json;
      req.write(jsonEncode(body));
    }
    return await req.close().timeout(timeout);
  }

  Future<HttpClientResponse> _delete(String path) async {
    final req = await _client.deleteUrl(Uri.parse('$baseUrl$path'));
    return await req.close().timeout(timeout);
  }

  Future<String> _readBody(HttpClientResponse res) async {
    return await res.transform<List<int>>(utf8.decoder).join();
  }

  // === Health ===
  Future<bool> ping() async {
    try {
      final res = await _get('/api/conversations');
      final body = await _readBody(res);
      return res.statusCode == 200;
    } catch (_) {
      return false;
    }
  }

  // === Conversations ===
  Future<List<Conversation>> getConversations() async {
    final res = await _get('/api/conversations');
    final body = await _readBody(res);
    final list = jsonDecode(body) as List;
    return list.map((j) => Conversation.fromJson(j)).toList();
  }

  Future<Conversation> createConversation(String title) async {
    final res = await _post('/api/conversations', body: {'title': title});
    final body = await _readBody(res);
    return Conversation.fromJson(jsonDecode(body));
  }

  Future<Conversation> getConversation(String id) async {
    final res = await _get('/api/conversations/$id');
    final body = await _readBody(res);
    return Conversation.fromJson(jsonDecode(body));
  }

  Future<void> deleteConversation(String id) async {
    await _delete('/api/conversations/$id');
  }

  Future<void> deleteMessage(String convId, String msgId) async {
    await _delete('/api/conversations/$convId/messages/$msgId');
  }

  Future<void> updateGroup(String convId, String group) async {
    await _put('/api/conversations/$convId/group', body: {'group': group});
  }

  // === Config ===
  Future<Map<String, dynamic>> getConfig() async {
    final res = await _get('/api/config');
    final body = await _readBody(res);
    return jsonDecode(body);
  }

  Future<void> updateConfig(Map<String, dynamic> config) async {
    await _put('/api/config', body: config);
  }

  // === Gateway ===
  Future<Map<String, dynamic>> getGateway() async {
    final res = await _get('/api/gateway');
    final body = await _readBody(res);
    return jsonDecode(body);
  }

  Future<List<KeyStatusInfo>> getKeyStatus() async {
    final res = await _get('/api/gateway/status');
    final body = await _readBody(res);
    final list = jsonDecode(body) as List;
    return list.map((j) => KeyStatusInfo.fromJson(j)).toList();
  }

  // === Skills ===
  Future<List<SkillInfo>> getSkills() async {
    final res = await _get('/api/skills');
    final body = await _readBody(res);
    final list = jsonDecode(body) as List;
    return list.map((j) => SkillInfo.fromJson(j)).toList();
  }

  // === Models ===
  Future<List<ModelInfo>> getModels() async {
    final res = await _get('/api/models');
    final body = await _readBody(res);
    final data = jsonDecode(body);
    final list = data['models'] as List? ?? [];
    return list.map((j) => ModelInfo.fromJson(j)).toList();
  }

  // === Stats ===
  Future<Map<String, dynamic>> getStats() async {
    final res = await _get('/api/stats');
    final body = await _readBody(res);
    return jsonDecode(body);
  }

  // === WebSocket ===
  WebSocketChannel connectWs() {
    return WebSocketChannel.connect(Uri.parse(wsUrl));
  }
}