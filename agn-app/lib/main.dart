/// AGN Studios — Flutter app entry point.
import 'package:flutter/material.dart';
import 'services/agn_api.dart';
import 'services/agn_theme.dart';
import 'screens/main_screen.dart';

void main() {
  runApp(AgnApp());
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