import 'package:re_highlight/languages/bash.dart';
import 'package:re_highlight/languages/c.dart';
import 'package:re_highlight/languages/cpp.dart';
import 'package:re_highlight/languages/csharp.dart';
import 'package:re_highlight/languages/css.dart';
import 'package:re_highlight/languages/dart.dart';
import 'package:re_highlight/languages/diff.dart';
import 'package:re_highlight/languages/dockerfile.dart';
import 'package:re_highlight/languages/go.dart';
import 'package:re_highlight/languages/graphql.dart';
import 'package:re_highlight/languages/java.dart';
import 'package:re_highlight/languages/javascript.dart';
import 'package:re_highlight/languages/json.dart';
import 'package:re_highlight/languages/kotlin.dart';
import 'package:re_highlight/languages/markdown.dart';
import 'package:re_highlight/languages/objectivec.dart';
import 'package:re_highlight/languages/php.dart';
import 'package:re_highlight/languages/powershell.dart';
import 'package:re_highlight/languages/python.dart';
import 'package:re_highlight/languages/ruby.dart';
import 'package:re_highlight/languages/rust.dart';
import 'package:re_highlight/languages/shell.dart';
import 'package:re_highlight/languages/sql.dart';
import 'package:re_highlight/languages/swift.dart';
import 'package:re_highlight/languages/typescript.dart';
import 'package:re_highlight/languages/xml.dart';
import 'package:re_highlight/languages/yaml.dart';
import 'package:re_highlight/re_highlight.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

class HighlightedHistoryCode {
  const HighlightedHistoryCode({required this.language, required this.span});

  final String language;
  final TextSpan span;
}

/// History-owned syntax presentation. It registers only the languages the UI
/// can name, keeping detection deterministic and the shipped grammar set small.
abstract final class HistoryCodeHighlighter {
  static const int _maxHighlightedCodeUnits = 200000;
  static const int _maxDetectionCodeUnits = 20000;
  static final Map<String, Mode> _languages = {
    'bash': langBash,
    'c': langC,
    'cpp': langCpp,
    'csharp': langCsharp,
    'css': langCss,
    'dart': langDart,
    'diff': langDiff,
    'dockerfile': langDockerfile,
    'go': langGo,
    'graphql': langGraphql,
    'java': langJava,
    'javascript': langJavascript,
    'json': langJson,
    'kotlin': langKotlin,
    'markdown': langMarkdown,
    'objectivec': langObjectivec,
    'php': langPhp,
    'powershell': langPowershell,
    'python': langPython,
    'ruby': langRuby,
    'rust': langRust,
    'shell': langShell,
    'sql': langSql,
    'swift': langSwift,
    'typescript': langTypescript,
    'xml': langXml,
    'yaml': langYaml,
  };

  static final Highlight _highlighter = Highlight()
    ..registerLanguages(_languages);
  static final List<String> _languageSubset = List.unmodifiable(
    _languages.keys,
  );

  static HighlightedHistoryCode highlight(
    String source,
    ThemeData theme, {
    bool json = false,
  }) {
    final detectionSource = source.length > _maxDetectionCodeUnits
        ? source.substring(0, _maxDetectionCodeUnits)
        : source;
    final hint = json ? 'json' : _languageHint(detectionSource);
    final detected = hint == null
        ? _highlighter.highlightAuto(detectionSource, _languageSubset)
        : _highlighter.highlight(code: detectionSource, language: hint);
    final base = theme.typography.mono
        .merge(theme.typography.small)
        .copyWith(color: theme.colorScheme.foreground, height: 1.45);
    if (source.length > _maxHighlightedCodeUnits) {
      return HighlightedHistoryCode(
        language: _languageLabel(detected.language),
        span: TextSpan(text: source, style: base),
      );
    }
    final result = source.length == detectionSource.length
        ? detected
        : detected.language == null
        ? _highlighter.justTextHighlightResult(source)
        : _highlighter.highlight(code: source, language: detected.language!);
    final renderer = TextSpanRenderer(base, _theme(theme));
    result.render(renderer);
    return HighlightedHistoryCode(
      language: _languageLabel(result.language),
      span: renderer.span ?? TextSpan(text: source, style: base),
    );
  }

  static String? _languageHint(String source) {
    final value = source.trimLeft();
    final lower = value.toLowerCase();
    if (lower.startsWith('<?php')) return 'php';
    if (lower.startsWith('<?xml') || lower.startsWith('<!doctype')) {
      return 'xml';
    }
    if (lower.startsWith('#!')) {
      return lower.startsWith('#!/usr/bin/env python') ? 'python' : 'bash';
    }
    if (RegExp(r'\bfn\s+\w+\s*\(').hasMatch(value) ||
        value.contains('println!') ||
        value.contains('let mut ') ||
        value.contains('impl ')) {
      return 'rust';
    }
    if (value.contains("import 'package:flutter/") ||
        value.contains("import 'dart:") ||
        value.contains('@override') ||
        value.contains('Widget build(')) {
      return 'dart';
    }
    if (lower.startsWith('package main') ||
        RegExp(r'\bfunc\s+\w+\s*\(').hasMatch(value) ||
        value.contains(':=')) {
      return 'go';
    }
    if (value.contains('#include') || value.contains('std::')) return 'cpp';
    if (value.contains('using System') || value.contains('Console.WriteLine')) {
      return 'csharp';
    }
    if (value.contains('public static void main') ||
        value.contains('System.out.println')) {
      return 'java';
    }
    if (RegExp(r'\bfun\s+\w+\s*\(').hasMatch(value) ||
        value.contains('val ') ||
        value.contains('data class ')) {
      return 'kotlin';
    }
    if (RegExp(r'\bfunc\s+\w+\s*\(').hasMatch(value) ||
        value.contains('import SwiftUI')) {
      return 'swift';
    }
    if (RegExp(
      r'^\s*(select|insert|update|delete|create)\b',
      caseSensitive: false,
    ).hasMatch(value)) {
      return 'sql';
    }
    if (RegExp(r'^\s*(def|class)\s+\w+', multiLine: true).hasMatch(value) ||
        lower.startsWith('from ') && lower.contains(' import ')) {
      return 'python';
    }
    if (RegExp(
      r'^\s*(git|npm|pnpm|yarn|cargo|docker|kubectl|brew|sudo)\s+',
    ).hasMatch(lower)) {
      return 'bash';
    }
    if (lower.startsWith('from ') && lower.contains('\nrun ')) {
      return 'dockerfile';
    }
    if (value.startsWith('<') && value.endsWith('>')) return 'xml';
    if (value.startsWith('# ') || value.startsWith('```')) return 'markdown';
    return null;
  }

  static Map<String, TextStyle> _theme(ThemeData theme) {
    final colors = theme.colorScheme;
    return {
      'doctag': TextStyle(color: colors.destructive),
      'keyword': TextStyle(color: colors.destructive),
      'meta-keyword': TextStyle(color: colors.destructive),
      'template-tag': TextStyle(color: colors.destructive),
      'template-variable': TextStyle(color: colors.destructive),
      'type': TextStyle(color: colors.destructive),
      'variable.language_': TextStyle(color: colors.destructive),
      'title': TextStyle(color: colors.chart4),
      'title.class_': TextStyle(color: colors.chart4),
      'title.class_.inherited__': TextStyle(color: colors.chart4),
      'title.function_': TextStyle(color: colors.chart4),
      'attr': TextStyle(color: colors.primary),
      'attribute': TextStyle(color: colors.primary),
      'literal': TextStyle(color: colors.primary),
      'meta': TextStyle(color: colors.primary),
      'number': TextStyle(color: colors.primary),
      'operator': TextStyle(color: colors.primary),
      'variable': TextStyle(color: colors.primary),
      'regexp': TextStyle(color: colors.chart2),
      'string': TextStyle(color: colors.chart2),
      'meta-string': TextStyle(color: colors.chart2),
      'built_in': TextStyle(color: colors.chart5),
      'symbol': TextStyle(color: colors.chart5),
      'comment': TextStyle(color: colors.mutedForeground),
      'code': TextStyle(color: colors.mutedForeground),
      'formula': TextStyle(color: colors.mutedForeground),
      'name': TextStyle(color: colors.chart1),
      'quote': TextStyle(color: colors.chart1),
      'selector-tag': TextStyle(color: colors.chart1),
      'section': TextStyle(color: colors.primary, fontWeight: FontWeight.bold),
      'emphasis': TextStyle(
        color: colors.foreground,
        fontStyle: FontStyle.italic,
      ),
      'strong': TextStyle(
        color: colors.foreground,
        fontWeight: FontWeight.bold,
      ),
    };
  }

  static String _languageLabel(String? language) => switch (language) {
    null => 'Code',
    'bash' => 'Bash',
    'cpp' => 'C++',
    'csharp' => 'C#',
    'css' => 'CSS',
    'dart' => 'Dart',
    'dockerfile' => 'Dockerfile',
    'go' => 'Go',
    'graphql' => 'GraphQL',
    'java' => 'Java',
    'javascript' => 'JavaScript',
    'json' => 'JSON',
    'kotlin' => 'Kotlin',
    'markdown' => 'Markdown',
    'objectivec' => 'Objective-C',
    'php' => 'PHP',
    'powershell' => 'PowerShell',
    'python' => 'Python',
    'ruby' => 'Ruby',
    'rust' => 'Rust',
    'shell' => 'Shell',
    'sql' => 'SQL',
    'swift' => 'Swift',
    'typescript' => 'TypeScript',
    'yaml' => 'YAML',
    'xml' => 'HTML/XML',
    final value => value,
  };
}
