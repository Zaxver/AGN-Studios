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

  AgnApi({this.baseUrl = 'http://127.0.0.1:11828'}) {
    wsUrl = baseUrl.replaceFirst('http', 'ws') + '/ws';
  }

  // === Conversations ===

  Future<List<Conversation>> getConversations() async {
    final r = await http.get(Uri.parse('$baseUrl/api/conversations'));
    if (r.statusCode != 200) throw Exception('Failed to load conversations');
    final list = jsonDecode(r.body) as List;
    return list.map((j) => Conversation.fromJson(j)).toList();
  }

  Future<Conversation> createConversation(String title) async {
    final r = await http.post(
      Uri.parse('$baseUrl/api/conversations'),
      headers: {'Content-Type': 'application/json'},
      body: jsonEncode({'title': title}),
    );
    return Conversation.fromJson(jsonDecode(r.body));
  }

  Future<Conversation> getConversation(String id) async {
    final r = await http.get(Uri.parse('$baseUrl/api/conversations/$id'));
    return Conversation.fromJson(jsonDecode(r.body));
  }

  Future<void> deleteConversation(String id) async {
    await http.delete(Uri.parse('$baseUrl/api/conversations/$id'));
  }

  Future<void> deleteMessage(String convId, String msgId) async {
    await http.delete(
      Uri.parse('$baseUrl/api/conversations/$convId/messages/$msgId'),
    );
  }

  Future<void> updateGroup(String convId, String group) async {
    await http.put(
      Uri.parse('$baseUrl/api/conversations/$convId/group'),
      headers: {'Content-Type': 'application/json'},
      body: jsonEncode({'group': group}),
    );
  }

  // === Config ===

  Future<Map<String, dynamic>> getConfig() async {
    final r = await http.get(Uri.parse('$baseUrl/api/config'));
    return jsonDecode(r.body);
  }

  Future<void> updateConfig(Map<String, dynamic> config) async {
    await http.put(
      Uri.parse('$baseUrl/api/config'),
      headers: {'Content-Type': 'application/json'},
      body: jsonEncode(config),
    );
  }

  // === Gateway ===

  Future<Map<String, dynamic>> getGateway() async {
    final r = await http.get(Uri.parse('$baseUrl/api/gateway'));
    return jsonDecode(r.body);
  }

  Future<List<KeyStatusInfo>> getKeyStatus() async {
    final r = await http.get(Uri.parse('$baseUrl/api/gateway/status'));
    final list = jsonDecode(r.body) as List;
    return list.map((j) => KeyStatusInfo.fromJson(j)).toList();
  }

  Future<void> addProvider(String name, String baseUrl, {String? protocol}) async {
    await http.post(
      Uri.parse('$baseUrl/api/gateway/providers'),
      headers: {'Content-Type': 'application/json'},
      body: jsonEncode({
        'name': name,
        'base_url': baseUrl,
        if (protocol != null) 'protocol': protocol,
      }),
    );
  }

  Future<void> addKeys(String providerId, String keys) async {
    await http.post(
      Uri.parse('$baseUrl/api/gateway/providers/$providerId/keys'),
      headers: {'Content-Type': 'application/json'},
      body: jsonEncode({'keys': keys}),
    );
  }

  Future<void> importGateway(String path) async {
    await http.post(
      Uri.parse('$baseUrl/api/gateway/import'),
      headers: {'Content-Type': 'application/json'},
      body: jsonEncode({'path': path}),
    );
  }

  // === Skills ===

  Future<List<SkillInfo>> getSkills() async {
    final r = await http.get(Uri.parse('$baseUrl/api/skills'));
    final list = jsonDecode(r.body) as List;
    return list.map((j) => SkillInfo.fromJson(j)).toList();
  }

  Future<void> installSkill(String url) async {
    await http.post(
      Uri.parse('$baseUrl/api/skills'),
      headers: {'Content-Type': 'application/json'},
      body: jsonEncode({'url': url}),
    );
  }

  Future<void> removeSkill(String name) async {
    await http.delete(Uri.parse('$baseUrl/api/skills/$name'));
  }

  Future<void> toggleSkill(String name, bool enabled) async {
    await http.put(
      Uri.parse('$baseUrl/api/skills/$name'),
      headers: {'Content-Type': 'application/json'},
      body: jsonEncode({'enabled': enabled}),
    );
  }

  // === Models ===

  Future<List<ModelInfo>> getModels() async {
    final r = await http.get(Uri.parse('$baseUrl/api/models'));
    final data = jsonDecode(r.body);
    final list = data['models'] as List? ?? [];
    return list.map((j) => ModelInfo.fromJson(j)).toList();
  }

  // === Stats ===

  Future<Map<String, dynamic>> getStats() async {
    final r = await http.get(Uri.parse('$baseUrl/api/stats'));
    return jsonDecode(r.body);
  }

  // === WebSocket ===

  WebSocketChannel connectWs() {
    return WebSocketChannel.connect(Uri.parse(wsUrl));
  }
}