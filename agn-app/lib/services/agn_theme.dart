/// AGN theme — OLED, dark, light (per spec).
import 'package:flutter/material.dart';

class AgnTheme {
  static ThemeData dark() => ThemeData(
    useMaterial3: true,
    brightness: Brightness.dark,
    colorScheme: const ColorScheme.dark(
      primary: Color(0xFFE94560),
      secondary: Color(0xFF533483),
      surface: Color(0xFF0F3460),
      surfaceContainerHighest: Color(0xFF1A1A3E),
      background: Color(0xFF1A1A2E),
    ),
    scaffoldBackgroundColor: const Color(0xFF1A1A2E),
    appBarTheme: const AppBarTheme(
      backgroundColor: Color(0xFF16213E),
      foregroundColor: Colors.white,
    ),
    inputDecorationTheme: const InputDecorationTheme(
      filled: true,
      fillColor: Color(0xFF0F3460),
      border: OutlineInputBorder(
        borderRadius: BorderRadius.all(Radius.circular(8)),
      ),
    ),
  );

  static ThemeData oled() => ThemeData(
    useMaterial3: true,
    brightness: Brightness.dark,
    colorScheme: const ColorScheme.dark(
      primary: Color(0xFFE94560),
      secondary: Color(0xFF533483),
      surface: Color(0xFF111111),
      surfaceContainerHighest: Color(0xFF1A1A1A),
      background: Color(0xFF000000),
    ),
    scaffoldBackgroundColor: Colors.black,
    appBarTheme: const AppBarTheme(
      backgroundColor: Color(0xFF0A0A0A),
      foregroundColor: Colors.white,
    ),
  );

  static ThemeData light() => ThemeData(
    useMaterial3: true,
    brightness: Brightness.light,
    colorScheme: const ColorScheme.light(
      primary: Color(0xFFE94560),
      secondary: Color(0xFF7B1FA2),
      surface: Colors.white,
      surfaceContainerHighest: Color(0xFFEEEEEE),
      background: Color(0xFFF5F5F5),
    ),
    scaffoldBackgroundColor: const Color(0xFFF5F5F5),
  );

  static ThemeData get(String name) {
    switch (name) {
      case 'oled':
        return oled();
      case 'light':
        return light();
      default:
        return dark();
    }
  }
}

/// Status colors.
class StatusColors {
  static const green = Color(0xFF4CAF50);
  static const yellow = Color(0xFFFFC107);
  static const red = Color(0xFFF44336);
}