/// AGN data models — mirror the Rust core types for the Flutter UI.

class Conversation {
  final String id;
  final String title;
  final List<ChatMessage> messages;
  final String group;
  final String createdAt;
  final String updatedAt;
  final String? workspace;

  Conversation({
    required this.id,
    required this.title,
    this.messages = const [],
    this.group = '',
    this.createdAt = '',
    this.updatedAt = '',
    this.workspace,
  });

  factory Conversation.fromJson(Map<String, dynamic> json) {
    return Conversation(
      id: json['id'] ?? '',
      title: json['title'] ?? 'New Chat',
      messages: (json['messages'] as List?)
          ?.map((m) => ChatMessage.fromJson(m))
          .toList() ??
          [],
      group: json['group'] ?? '',
      createdAt: json['created_at'] ?? '',
      updatedAt: json['updated_at'] ?? '',
      workspace: json['workspace'],
    );
  }
}

class ChatMessage {
  final String id;
  final String role;
  final String content;
  final List<ToolCall>? toolCalls;
  final String? toolCallId;
  final TokenUsage? usage;
  final String timestamp;

  ChatMessage({
    required this.id,
    required this.role,
    required this.content,
    this.toolCalls,
    this.toolCallId,
    this.usage,
    this.timestamp = '',
  });

  factory ChatMessage.fromJson(Map<String, dynamic> json) {
    return ChatMessage(
      id: json['id'] ?? '',
      role: json['role'] ?? 'user',
      content: json['content'] ?? '',
      toolCalls: (json['tool_calls'] as List?)
          ?.map((tc) => ToolCall.fromJson(tc))
          .toList(),
      toolCallId: json['tool_call_id'],
      usage: json['usage'] != null ? TokenUsage.fromJson(json['usage']) : null,
      timestamp: json['timestamp'] ?? '',
    );
  }
}

class ToolCall {
  final String id;
  final String name;
  final dynamic arguments;
  final String? result;
  final bool? success;

  ToolCall({
    required this.id,
    required this.name,
    required this.arguments,
    this.result,
    this.success,
  });

  factory ToolCall.fromJson(Map<String, dynamic> json) {
    return ToolCall(
      id: json['id'] ?? '',
      name: json['name'] ?? '',
      arguments: json['arguments'],
      result: json['result'],
      success: json['success'],
    );
  }
}

class TokenUsage {
  final int promptTokens;
  final int completionTokens;
  final int totalTokens;
  final int? timeToFirstTokenMs;
  final double? tokensPerSecond;

  TokenUsage({
    required this.promptTokens,
    required this.completionTokens,
    required this.totalTokens,
    this.timeToFirstTokenMs,
    this.tokensPerSecond,
  });

  factory TokenUsage.fromJson(Map<String, dynamic> json) {
    return TokenUsage(
      promptTokens: (json['prompt_tokens'] ?? 0) as int,
      completionTokens: (json['completion_tokens'] ?? 0) as int,
      totalTokens: (json['total_tokens'] ?? 0) as int,
      timeToFirstTokenMs: json['time_to_first_token_ms']?.toInt(),
      tokensPerSecond: (json['tokens_per_second'] as num?)?.toDouble(),
    );
  }
}

/// Conversation status shown in sidebar (green/yellow/red dot).
enum ConvStatus { idle, generating, waitingUser, retrying, networkRetry, error }

/// Key status for the gateway key pool panel.
class KeyStatusInfo {
  final String providerName;
  final String keyId;
  final String keyLabel;
  final String status; // available, cooling, networkerror, invalid, unused
  final int? cooldownRemaining;
  final String? lastError;

  KeyStatusInfo({
    required this.providerName,
    required this.keyId,
    required this.keyLabel,
    required this.status,
    this.cooldownRemaining,
    this.lastError,
  });

  factory KeyStatusInfo.fromJson(Map<String, dynamic> json) {
    return KeyStatusInfo(
      providerName: json['provider_name'] ?? '',
      keyId: json['key_id'] ?? '',
      keyLabel: json['key_label'] ?? '',
      status: json['status'] ?? 'unused',
      cooldownRemaining: json['cooldown_remaining']?.toInt(),
      lastError: json['last_error'],
    );
  }
}

/// Model info from /api/models.
class ModelInfo {
  final String id;
  final String provider;
  final int? contextLength;
  final bool supportsImages;

  ModelInfo({
    required this.id,
    required this.provider,
    this.contextLength,
    this.supportsImages = false,
  });

  factory ModelInfo.fromJson(Map<String, dynamic> json) {
    return ModelInfo(
      id: json['id'] ?? '',
      provider: json['provider'] ?? '',
      contextLength: json['context_length']?.toInt(),
      supportsImages: json['supports_images'] ?? false,
    );
  }
}

/// Skill info from /api/skills.
class SkillInfo {
  final String name;
  final String description;
  final String version;
  final bool enabled;

  SkillInfo({
    required this.name,
    required this.description,
    required this.version,
    required this.enabled,
  });

  factory SkillInfo.fromJson(Map<String, dynamic> json) {
    return SkillInfo(
      name: json['name'] ?? '',
      description: json['description'] ?? '',
      version: json['version'] ?? '',
      enabled: json['enabled'] ?? true,
    );
  }
}

/// Gateway config.
class ProviderInfo {
  final String id;
  final String name;
  final String baseUrl;
  final int keyCount;

  ProviderInfo({
    required this.id,
    required this.name,
    required this.baseUrl,
    this.keyCount = 0,
  });

  factory ProviderInfo.fromJson(Map<String, dynamic> json) {
    return ProviderInfo(
      id: json['id'] ?? '',
      name: json['name'] ?? '',
      baseUrl: json['base_url'] ?? '',
      keyCount: (json['keys'] as List?)?.length ?? 0,
    );
  }
}

/// Generation status shown during LLM response.
class GenStatus {
  final String state; // generating, retrying, network, waiting_user, error, idle
  final int? current;
  final int? total;
  final String? message;

  GenStatus({required this.state, this.current, this.total, this.message});

  factory GenStatus.fromJson(Map<String, dynamic> json) {
    return GenStatus(
      state: json['state'] ?? 'idle',
      current: json['current']?.toInt(),
      total: json['total']?.toInt(),
      message: json['message'],
    );
  }
}