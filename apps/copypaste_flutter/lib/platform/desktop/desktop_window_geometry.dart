import 'dart:math' as math;

const desktopWindowInitialBounds = DesktopWindowBounds(
  left: 0,
  top: 0,
  width: 1100,
  height: 760,
);

const desktopWindowMinimumSize = DesktopWindowSize(width: 360, height: 480);

class DesktopWindowSize {
  const DesktopWindowSize({required this.width, required this.height});

  final double width;
  final double height;
}

class DesktopWindowBounds {
  const DesktopWindowBounds({
    required this.left,
    required this.top,
    required this.width,
    required this.height,
  });

  final double left;
  final double top;
  final double width;
  final double height;

  bool get isValid =>
      left.isFinite &&
      top.isFinite &&
      width.isFinite &&
      height.isFinite &&
      width > 0 &&
      height > 0;
}

class DesktopWorkArea {
  const DesktopWorkArea({
    required this.left,
    required this.top,
    required this.width,
    required this.height,
  });

  final double left;
  final double top;
  final double width;
  final double height;

  double get right => left + width;
  double get bottom => top + height;
}

abstract interface class DesktopWindowGeometryStore {
  Future<DesktopWindowBounds?> read();

  Future<void> write(DesktopWindowBounds bounds);
}

DesktopWindowSize minimumSizeFor(DesktopWorkArea workArea) {
  return DesktopWindowSize(
    width: math.min(desktopWindowMinimumSize.width, workArea.width),
    height: math.min(desktopWindowMinimumSize.height, workArea.height),
  );
}

/// Restores a saved logical-pixel window rectangle inside a visible work area.
DesktopWindowBounds restoreDesktopWindowBounds({
  required DesktopWorkArea workArea,
  DesktopWindowBounds? savedBounds,
}) {
  final maximumWidth = math.max(0.0, workArea.width);
  final maximumHeight = math.max(0.0, workArea.height);
  final minimumSize = minimumSizeFor(workArea);
  final saved = savedBounds?.isValid == true ? savedBounds : null;
  final width = _clampSize(
    saved?.width ?? desktopWindowInitialBounds.width,
    minimumSize.width,
    maximumWidth,
  );
  final height = _clampSize(
    saved?.height ?? desktopWindowInitialBounds.height,
    minimumSize.height,
    maximumHeight,
  );
  final centeredLeft = workArea.left + (maximumWidth - width) / 2;
  final centeredTop = workArea.top + (maximumHeight - height) / 2;
  final left = _clamp(
    saved?.left ?? centeredLeft,
    workArea.left,
    workArea.right - width,
  );
  final top = _clamp(
    saved?.top ?? centeredTop,
    workArea.top,
    workArea.bottom - height,
  );

  return DesktopWindowBounds(
    left: left,
    top: top,
    width: width,
    height: height,
  );
}

double _clampSize(double value, double minimum, double maximum) {
  if (maximum <= minimum) {
    return maximum;
  }
  return _clamp(value, minimum, maximum);
}

double _clamp(double value, double minimum, double maximum) {
  if (maximum <= minimum) {
    return minimum;
  }
  return math.max(minimum, math.min(value, maximum));
}
