import 'dart:async';
import 'dart:convert';
import 'dart:ffi';
import 'dart:io';

import 'package:crypto/crypto.dart';
import 'package:cryptography/cryptography.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:flutter/services.dart';
import 'package:pub_semver/pub_semver.dart';
import 'package:copypaste_flutter/features/modules/models/module_marketplace_models.dart';
import 'package:copypaste_flutter/features/modules/models/module_models.dart';
import 'package:copypaste_flutter/features/modules/repository/github_module_marketplace_repository.dart';
import 'package:copypaste_flutter/platform/modules/module_marketplace_platform.dart';
import 'package:copypaste_flutter/platform/update/app_update_platform.dart';
import 'package:copypaste_flutter/shared/security/minisign_verifier.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  final target = ModuleMarketplaceTarget(
    platform: 'macos',
    architecture: 'aarch64',
    appVersion: Version.parse('1.0.3'),
  );
  Map<String, Object> catalog() => {
    'schema_version': 1,
    'modules': [
      {
        'id': 'copypaste.ocr',
        'title': 'OCR',
        'description': 'Recognize images offline.',
        'version': '0.1.0',
        'app_versions': '>=1.0.0, <2.0.0',
        'artifacts': [
          for (final platform in ['macos', 'windows', 'android'])
            for (final architecture in ['aarch64', 'x86_64', 'arm'])
              {
                'platform': platform,
                'architecture': architecture,
                'url':
                    'https://github.com/dmytro-yevs/copypaste/releases/download/module-copypaste.ocr-v0.1.0/ocr-$platform-$architecture.cpmodule',
                'size_bytes': 7,
                'sha256': sha256.convert(utf8.encode('package')).toString(),
              },
        ],
      },
    ],
  };

  test(
    'selects exact platform, process architecture, and app compatibility',
    () {
      for (final platform in ['macos', 'windows', 'android']) {
        for (final architecture in ['aarch64', 'x86_64', 'arm']) {
          final modules = const ModuleCatalogParser().parse(
            jsonEncode(catalog()),
            ModuleMarketplaceTarget(
              platform: platform,
              architecture: architecture,
              appVersion: target.appVersion,
            ),
          );
          expect(
            modules.single.artifact!.downloadUri.path,
            endsWith('ocr-$platform-$architecture.cpmodule'),
          );
        }
      }
      for (final version in ['0.9.0', '2.0.0']) {
        expect(
          const ModuleCatalogParser().parse(
            jsonEncode(catalog()),
            ModuleMarketplaceTarget(
              platform: 'macos',
              architecture: 'aarch64',
              appVersion: Version.parse(version),
            ),
          ),
          hasLength(1),
        );
      }
      expect(
        const ModuleCatalogParser().parse(
          jsonEncode(catalog()),
          ModuleMarketplaceTarget(
            platform: 'android',
            architecture: 'x86',
            appVersion: target.appVersion,
          ),
        ),
        hasLength(1),
      );
    },
  );

  test(
    'keeps incompatible modules visible with exact app and OS requirements',
    () {
      final body = catalog();
      final module = (body['modules'] as List).first as Map;
      module['app_versions'] = '>=1.0.6, <2.0.0';
      for (final artifact in module['artifacts'] as List) {
        (artifact as Map)['minimum_system_version'] = '14.0.0';
      }
      final oldApp = const ModuleCatalogParser()
          .parse(
            jsonEncode(body),
            ModuleMarketplaceTarget(
              platform: 'macos',
              architecture: 'aarch64',
              appVersion: Version.parse('1.0.4'),
              systemVersion: Version.parse('26.0.0'),
            ),
          )
          .single;
      expect(oldApp.availability, ModuleAvailability.appVersion);
      expect(oldApp.canInstall, isFalse);
      expect(oldApp.appRequirement, contains('1.0.6'));
      final oldSystem = const ModuleCatalogParser()
          .parse(
            jsonEncode(body),
            ModuleMarketplaceTarget(
              platform: 'macos',
              architecture: 'aarch64',
              appVersion: Version.parse('1.0.6'),
              systemVersion: Version.parse('13.0.0'),
            ),
          )
          .single;
      expect(oldSystem.availability, ModuleAvailability.systemVersion);
      expect(oldSystem.unavailableReason, 'Requires macOS 14 or newer.');
      expect(oldSystem.canInstall, isFalse);
      final compatible = const ModuleCatalogParser()
          .parse(
            jsonEncode(body),
            ModuleMarketplaceTarget(
              platform: 'macos',
              architecture: 'aarch64',
              appVersion: Version.parse('1.0.6'),
              systemVersion: Version.parse('14.0.0'),
            ),
          )
          .single;
      expect(compatible.canInstall, isTrue);
      final unknownApp = const ModuleCatalogParser()
          .parse(
            jsonEncode(body),
            const ModuleMarketplaceTarget(
              platform: 'macos',
              architecture: 'aarch64',
              appVersion: null,
            ),
          )
          .single;
      expect(unknownApp.canInstall, isFalse);
      expect(unknownApp.availability, ModuleAvailability.appVersion);
    },
  );

  test(
    'reads native versions and preserves app prerelease compatibility',
    () async {
      const channel = MethodChannel('test/modules/system-version');
      TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
          .setMockMethodCallHandler(
            channel,
            (call) async =>
                call.method == 'systemVersion' ? '16' : '1.0.6-beta.1',
          );
      addTearDown(
        () => TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
            .setMockMethodCallHandler(channel, null),
      );
      final platform = ModuleMarketplacePlatform(
        appPlatform: MethodChannelAppUpdatePlatform(channel: channel),
        systemChannel: channel,
        currentAbi: () => Abi.androidArm64,
      );
      final versions = await platform.currentTarget();
      expect(versions.appVersion, Version.parse('1.0.6-beta.1'));
      expect(versions.systemVersion, Version.parse('16.0.0'));
      TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
          .setMockMethodCallHandler(channel, (call) async => null);
      final unknown = await platform.currentTarget();
      expect(unknown.appVersion, isNull);
      expect(unknown.systemVersion, isNull);
    },
  );

  test(
    'rejects duplicate modules, duplicate targets, invalid metadata and foreign URLs',
    () {
      final malformed = <Map<String, Object>>[];
      final duplicate = catalog();
      (duplicate['modules'] as List).add((duplicate['modules'] as List).first);
      malformed.add(duplicate);
      for (final mutation in [
        (Map module) => module['version'] = '0.2.0-beta',
        (Map module) => module['title'] = '',
        (Map module) => module['id'] = 'third-party.ocr',
        (Map module) => (module['artifacts'] as List).add(
          (module['artifacts'] as List).first,
        ),
        (Map module) => (module['artifacts'] as List).first['url'] =
            'https://example.com/ocr.cpmodule',
        (Map module) => (module['artifacts'] as List).first['size_bytes'] =
            maximumModulePackageBytes + 1,
        (Map module) =>
            (module['artifacts'] as List).first['sha256'] = 'incorrect',
      ]) {
        final value = catalog();
        mutation((value['modules'] as List).first as Map);
        malformed.add(value);
      }
      for (final value in malformed) {
        expect(
          () => const ModuleCatalogParser().parse(jsonEncode(value), target),
          throwsA(isA<ModulesException>()),
        );
      }
    },
  );

  test(
    'maps all supported process ABIs without guessing from the device name',
    () {
      final expected = {
        Abi.macosArm64: ('macos', 'aarch64'),
        Abi.macosX64: ('macos', 'x86_64'),
        Abi.windowsArm64: ('windows', 'aarch64'),
        Abi.windowsX64: ('windows', 'x86_64'),
        Abi.windowsIA32: ('windows', 'x86'),
        Abi.androidArm64: ('android', 'aarch64'),
        Abi.androidArm: ('android', 'arm'),
        Abi.androidX64: ('android', 'x86_64'),
        Abi.androidIA32: ('android', 'x86'),
      };
      for (final entry in expected.entries) {
        expect(ModuleMarketplacePlatform.targetForAbi(entry.key), entry.value);
      }
      expect(
        () => ModuleMarketplacePlatform.targetForAbi(Abi.linuxX64),
        throwsA(isA<ModulesException>()),
      );
    },
  );

  test(
    'creates missing cache directories, authenticates downloads, and cleans private staging',
    () async {
      final client = _Client();
      final directory = await Directory.systemTemp.createTemp(
        'marketplace-test-',
      );
      addTearDown(() => directory.delete(recursive: true));
      final cache = Directory('${directory.path}/missing/cache');
      expect(await cache.exists(), isFalse);
      final body = utf8.encode(jsonEncode(catalog()));
      final (publicKey, signature) = await _sign(body);
      client.routes['modules.json'] = _Response(body);
      client.routes['modules.json.sig'] = _Response(utf8.encode(signature));
      final repository = GitHubModuleMarketplaceRepository(
        temporaryDirectory: () async => cache,
        currentTarget: () async => target,
        client: client,
        signatureVerifier: MinisignVerifier(publicKeyBase64: publicKey),
      );
      addTearDown(repository.dispose);
      final module = (await repository.list()).single;
      expect(await cache.exists(), isTrue);
      expect(await cache.list().toList(), isEmpty);
      await cache.delete(recursive: true);
      client.routes[module.artifact!.downloadUri.pathSegments.last] = _Response(
        utf8.encode('package'),
      );
      final progress = <double>[];
      final package = await repository.download(
        module,
        onProgress: progress.add,
      );
      expect(await File(package.path).readAsString(), 'package');
      expect(progress.last, 1);
      await package.dispose();
      expect(await cache.exists(), isTrue);
      expect(await cache.list().toList(), isEmpty);

      client.routes[module.artifact!.downloadUri.pathSegments.last] = _Response(
        utf8.encode('changed'),
      );
      await expectLater(
        repository.download(module, onProgress: (_) {}),
        throwsA(isA<ModulesException>()),
      );
      expect(await cache.list().toList(), isEmpty);
      client.routes['modules.json'] = _Response(
        utf8.encode(jsonEncode({'schema_version': 1, 'modules': []})),
      );
      await expectLater(repository.list(), throwsA(isA<ModulesException>()));
      expect(await cache.list().toList(), isEmpty);
    },
  );

  test(
    'rejects untrusted redirects and oversized streams before installation',
    () async {
      final directory = await Directory.systemTemp.createTemp(
        'marketplace-network-',
      );
      addTearDown(() => directory.delete(recursive: true));
      final client = _Client();
      final repository = GitHubModuleMarketplaceRepository(
        temporaryDirectory: () async => directory,
        currentTarget: () async => target,
        client: client,
      );
      addTearDown(repository.dispose);
      client.routes['modules.json'] = _Response(
        [],
        statusCode: 302,
        location: 'http://example.com/catalog',
      );
      await expectLater(repository.list(), throwsA(isA<ModulesException>()));
      expect(client.requests, hasLength(1));
      client.routes['modules.json'] = _Response(
        List.filled(2 * 1024 * 1024 + 1, 0),
        contentLength: -1,
      );
      await expectLater(repository.list(), throwsA(isA<ModulesException>()));
      expect(await directory.list().toList(), isEmpty);
      client.routes['modules.json'] = _Response([], statusCode: 404);
      expect(await repository.list(), isEmpty);
      expect(await directory.list().toList(), isEmpty);
    },
  );
}

Future<(String, String)> _sign(List<int> data) async {
  final algorithm = Ed25519();
  final pair = await algorithm.newKeyPair();
  final publicKey = await pair.extractPublicKey();
  final keyId = List.filled(8, 1);
  final digest = await Blake2b(hashLengthInBytes: 64).hash(data);
  final signature = await algorithm.sign(digest.bytes, keyPair: pair);
  const comment = 'timestamp:1\tfile:modules.json';
  final global = await algorithm.sign([
    ...signature.bytes,
    ...utf8.encode(comment),
  ], keyPair: pair);
  final envelope =
      'untrusted comment: test\n${base64.encode([0x45, 0x44, ...keyId, ...signature.bytes])}\ntrusted comment: $comment\n${base64.encode(global.bytes)}';
  return (
    base64.encode([0x45, 0x64, ...keyId, ...publicKey.bytes]),
    base64.encode(utf8.encode(envelope)),
  );
}

class _Client implements HttpClient {
  final routes = <String, _Response>{};
  final requests = <Uri>[];
  @override
  Future<HttpClientRequest> getUrl(Uri url) async {
    requests.add(url);
    return _Request(routes[url.pathSegments.last]!);
  }

  @override
  void close({bool force = false}) {}
  @override
  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);
}

class _Request implements HttpClientRequest {
  _Request(this.response);
  final _Response response;
  @override
  final HttpHeaders headers = _Headers();
  @override
  bool followRedirects = true;
  @override
  Future<HttpClientResponse> close() async => response;
  @override
  void abort([Object? exception, StackTrace? stackTrace]) {}
  @override
  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);
}

class _Headers implements HttpHeaders {
  _Headers([this.location]);
  final String? location;
  @override
  void set(String name, Object value, {bool preserveHeaderCase = false}) {}
  @override
  String? value(String name) =>
      name == HttpHeaders.locationHeader ? location : null;
  @override
  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);
}

class _Response extends Stream<List<int>> implements HttpClientResponse {
  _Response(
    this.bytes, {
    this.statusCode = 200,
    int? contentLength,
    String? location,
  }) : contentLength = contentLength ?? bytes.length,
       headers = _Headers(location);
  final List<int> bytes;
  @override
  final int contentLength;
  @override
  final int statusCode;
  @override
  final HttpHeaders headers;
  @override
  StreamSubscription<List<int>> listen(
    void Function(List<int>)? onData, {
    Function? onError,
    void Function()? onDone,
    bool? cancelOnError,
  }) => Stream<List<int>>.fromIterable([bytes]).listen(
    onData,
    onError: onError,
    onDone: onDone,
    cancelOnError: cancelOnError,
  );
  @override
  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);
}
