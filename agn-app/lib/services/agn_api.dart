/// AGN API client — talks to the Rust backend (agn serve).
///
/// Default API: http://127.0.0.1:11828
/// Default WebUI: http://127.0.0.1:11829

import 'dart:convert';
import 'package:http/http.dart' as http;
import 'package:web_socket_channel/web_socket_channel.dart';
import '../models/agn_models.dart';

class AgnApi {
  String baseUrl;
  late final String wsUrl;
  Duration timeout = const Duration(seconds: 5);

  AgnApi({this.baseUrl = 'http://127.0.0.1:11828'}) {
    wsUrl = baseUrl.replaceFirst('http', 'ws') + '/ws';
  }

  // === Health ===
  Future<bool> ping() async {
    try {
      final r = await http.get(Uri.parse('$baseUrl/api/conversations')).timeout(timeout);
      return r.statusCode == 200;
    } catch (_) {
      return false;
    }
  }

  // === Conversations ===

  Future<List<Conversation>> getConversations() async {
    final r = await http.get(Uri.parse('$baseUrl/api/conversations')).timeout(timeout);
    if (r.statusCode != 200) throw Exception('Failed to load conversations');
    final list = jsonDecode(r.body) as List;
    return list.map((j) => Conversation.fromJson(j)).toList();
  }

  Future<Conversation> createConversation(String title) async {
    final r = await http.post(
      Uri.parse('$baseUrl/api/conversations'),
      headers: {'Content-Type': 'application/json'},
      body: jsonEncode({'title': title}),
    ).timeout(timeout);
    return Conversation.fromJson(jsonDecode(r.body));
  }

  Future<Conversation> getConversation(String id) async {
    final r = await http.get(Uri.parse('$baseUrl/api/conversations/$id')).timeout(timeout);
    return Conversation.fromJson(jsonDecode(r.body));
  }

  Future<void> deleteConversation(String id) async {
    await http.delete(Uri.parse('$baseUrl/api/conversations/$id')).timeout(timeout);
  }

  Future<void> deleteMessage(String convId, String msgId) async {
    await http.delete(
      Uri.parse('$baseUrl/api/conversations/$convId/messages/$msgId'),
    ).timeout(timeout);
  }

  Future<void> updateGroup(String convId, String group) async {
    await http.put(
      Uri.parse('$baseUrl/api/conversations/$convId/group'),
      headers: {'Content-Type': 'application/json'},
      body: jsonEncode({'group': group}),
    ).timeout(timeout);
  }

  // === Config ===

  Future<Map<String, dynamic>> getConfig() async {
    final r = await http.get(Uri.parse('$baseUrl/api/config')).timeout(timeout);
    return jsonDecode(r.body);
  }

  Future<void> updateConfig(Map<String, dynamic> config) async {
    await http.put(
      Uri.parse('$baseUrl/api/config'),
      headers: {'Content-Type': 'application/json'},
      body: jsonEncode(config),
    ).timeout(timeout);
  }

  // === Gateway ===

  Future<Map<String, dynamic>> getGateway() async {
    final r = await http.get(Uri.parse('$baseUrl/api/gateway')).timeout(timeout);
    return jsonDecode(r.body);
  }

  Future<List<KeyStatusInfo>> getKeyStatus() async {
    final r = await http.get(Uri.parse('$baseUrl/api/gateway/status')).timeout(timeout);
    final list = jsonDecode(r.body) as List;
    return list.map((j) => KeyStatusInfo.fromJson(j)).toList();
  }

  // === Skills ===

  Future<List<SkillInfo>> getSkills() async {
    final r = await http.get(Uri.parse('$baseUrl/api/skills')).timeout(timeout);
    final list = jsonDecode(r.body) as List;
    return list.map((j) => SkillInfo.fromJson(j)).toList();
  }

  // === Models ===

  Future<List<ModelInfo>> getModels() async {
    final r = await http.get(Uri.parse('$baseUrl/api/models')).timeout(timeout);
    final data = jsonDecode(r.body);
    final list = data['models'] as List? ?? [];
    return list.map((j) => ModelInfo.fromJson(j)).toList();
  }

  // === Stats ===

  Future<Map<String, dynamic>> getStats() async {
    final r = await http.get(Uri.parse('$baseUrl/api/stats')).timeout(timeout);
    return jsonDecode(r.body);
  }

  // === WebSocket ===

  WebSocketChannel connectWs() {
    return WebSocketChannel.connect(Uri.parse(wsUrl));
  }
}