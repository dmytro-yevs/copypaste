import 'dart:io';

// A separate OS process is required to exercise POSIX advisory-lock lifetime.
Future<void> main(List<String> arguments) async {
  final directory = arguments.single;
  final file = await File('$directory/.copypaste-active')
      .open(mode: FileMode.append);
  await file.lock();
  await File('$directory/.copypaste-owner').writeAsString('$pid');
  stdout.writeln('ready');
  await stdin.first;
  await file.close();
}
