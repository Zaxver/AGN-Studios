/// AGN Studios — Flutter app entry point with full logging.
import 'dart:io';
import 'dart:async';
import 'package:flutter/material.dart';
import 'package:path_provider/path_provider.dart';
import 'services/agn_api.dart';
import 'services/agn_theme.dart';
import 'screens/main_screen.dart';

final _logBuffer = StringBuffer();

void _log(String msg) {
  final ts = DateTime.now().toIso8601String();
  final line = '[$ts] $msg';
  _logBuffer.writeln(line);
  debugPrint(line);
}

Future<void> _flushLog() async {
  try {
    // Write to public Downloads directory for easy access
    final f = File('/storage/emulated/0/Download/agn_app.log');
    await f.writeAsString(_logBuffer.toString());
    _log('Log written to ${f.path}');
  } catch (e) {
    _log('Log write failed: $e');
    // Fallback to app documents
    try {
      final dir = await getApplicationDocumentsDirectory();
      final f = File('${dir.path}/agn_app.log');
      await f.writeAsString(_logBuffer.toString());
      _log('Log written (fallback) to ${f.path}');
    } catch (e2) {
      _log('Log write fallback failed: $e2');
    }
  }
}

void main() {
  runZonedGuarded(() async {
    WidgetsFlutterBinding.ensureInitialized();

    // Allow cleartext HTTP
    HttpOverrides.global = _MyHttpOverrides();

    _log('=== AGN App Start ===');
    _log('Platform: ${Platform.operatingSystem}');
    _log('IsAndroid: ${Platform.isAndroid}');

    // Test HTTP connectivity
    try {
      _log('Testing HTTP to http://127.0.0.1:11828/api/conversations ...');
      final client = HttpClient();
      client.badCertificateCallback = (cert, host, port) => true;
      final req = await client.getUrl(Uri.parse('http://127.0.0.1:11828/api/conversations'));
      _log('Request sent, waiting response...');
      final res = await req.close();
      final body = await res.transform<String>(SystemEncoding().decoder).join();
      _log('HTTP test: status=${res.statusCode} body=${body.substring(0, body.length > 200 ? 200 : body.length)}');
      client.close();
    } catch (e, st) {
      _log('HTTP test FAILED: $e');
      _log('Stack: $st');
    }

    // Also test with package:http
    try {
      _log('Testing package:http...');
      final r = await AgnApi(baseUrl: 'http://127.0.0.1:11828').ping();
      _log('AgnApi.ping() result: $r');
    } catch (e, st) {
      _log('AgnApi.ping() FAILED: $e');
      _log('Stack: $st');
    }

    await _flushLog();

    runApp(AgnApp());
  }, (error, stack) {
    _log('ZONE ERROR: $error');
    _log('ZONE STACK: $stack');
    _flushLog();
  });
}

class _MyHttpOverrides extends HttpOverrides {
  @override
  HttpClient createHttpClient(SecurityContext? context) {
    final client = super.createHttpClient(context);
    client.badCertificateCallback = (cert, host, port) => true;
    return client;
  }
}

class AgnApp extends StatelessWidget {
  @override
  Widget build(BuildContext context) {
    final api = AgnApi(baseUrl: 'http://127.0.0.1:11828');
    return MaterialApp(
      title: 'AGN Studios',
      debugShowCheckedModeBanner: false,
      theme: AgnTheme.dark(),
      home: MainScreen(api: api),
    );
  }
}