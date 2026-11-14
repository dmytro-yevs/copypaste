import 'package:flutter/foundation.dart';

/// An application a setting can name, by the identifier capture attribution
/// records for it on this platform. [iconId] names a clip that carries the
/// icon captured with it, when History has one.
@immutable
class ApplicationChoice {
  const ApplicationChoice({required this.id, required this.name, this.iconId});

  final String id;
  final String name;
  final String? iconId;
}
