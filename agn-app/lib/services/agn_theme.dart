/// AGN theme — OLED black by default.
import 'package:flutter/material.dart';

class AgnTheme {
  static ThemeData dark() => ThemeData(
    useMaterial3: true,
    brightness: Brightness.dark,
    colorScheme: const ColorScheme.dark(
      primary: Color(0xFFE94560),
      onPrimary: Colors.white,
      secondary: Color(0xFF533483),
      onSecondary: Colors.white,
      surface: Color(0xFF111111),
      onSurface: Color(0xFFE0E0E0),
      surfaceContainerLow: Color(0xFF1A1A1A),
      surfaceContainer: Color(0xFF1E1E1E),
      surfaceContainerHigh: Color(0xFF242424),
      surfaceContainerHighest: Color(0xFF2A2A2A),
      outline: Color(0xFF3A3A3A),
      outlineVariant: Color(0xFF2A2A2A),
    ),
    scaffoldBackgroundColor: Colors.black,
    canvasColor: Colors.black,
    appBarTheme: const AppBarTheme(
      backgroundColor: Colors.black,
      foregroundColor: Colors.white,
      elevation: 0,
    ),
    inputDecorationTheme: const InputDecorationTheme(
      filled: true,
      fillColor: Colors.transparent,
      border: InputBorder.none,
      enabledBorder: InputBorder.none,
      focusedBorder: InputBorder.none,
    ),
    dividerTheme: const DividerThemeData(
      color: Color(0xFF2A2A2A),
      thickness: 0.5,
    ),
  );

  static ThemeData oled() => dark();

  static ThemeData light() => ThemeData(
    useMaterial3: true,
    brightness: Brightness.light,
    colorScheme: const ColorScheme.light(
      primary: Color(0xFFE94560),
      secondary: Color(0xFF7B1FA2),
      surface: Colors.white,
    ),
    scaffoldBackgroundColor: const Color(0xFFF5F5F5),
  );

  static ThemeData get(String name) {
    switch (name) {
      case 'light':
        return light();
      default:
        return dark();
    }
  }
}

class StatusColors {
  static const green = Color(0xFF4CAF50);
  static const yellow = Color(0xFFFFC107);
  static const red = Color(0xFFF44336);
}