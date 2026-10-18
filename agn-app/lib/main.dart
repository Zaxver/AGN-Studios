/// AGN Studios — Flutter app entry point.
import 'dart:io';
import 'package:flutter/material.dart';
import 'services/agn_api.dart';
import 'services/agn_theme.dart';
import 'screens/main_screen.dart';

void main() {
  // Allow cleartext HTTP for localhost
  HttpOverrides.global = _MyHttpOverrides();
  runApp(AgnApp());
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