#!/usr/bin/env python3
"""Construct the packaged camera_desktop capture pipelines without opening a device."""

from __future__ import annotations

import argparse
import ctypes
from pathlib import Path
import sys


PREVIEW = (
    "! videoflip name=flip method=horizontal-flip ! videoscale ! videorate "
    "! video/x-raw,format=RGBA,width=640,height=480,framerate=30/1 "
    "! tee name=t t. ! queue name=preview_queue "
    "! appsink name=sink emit-signals=true max-buffers=2 drop=true sync=false"
)
PIPELINES = {
    "raw": f"v4l2src device=/dev/null ! videoconvert {PREVIEW}",
    "mjpeg": f"v4l2src device=/dev/null ! image/jpeg,width=640,height=480 ! jpegdec ! videoconvert {PREVIEW}",
}


class PipelineError(RuntimeError):
    pass


class _GError(ctypes.Structure):
    _fields_ = [("domain", ctypes.c_uint32), ("code", ctypes.c_int), ("message", ctypes.c_char_p)]


def runtime_library(runtime: Path, soname: str) -> Path:
    try:
        root = runtime.resolve(strict=True)
        source = (root / soname).resolve(strict=True)
        source.relative_to(root)
    except (OSError, ValueError) as error:
        raise PipelineError("camera pipeline library is absent or outside the packaged runtime") from error
    if not source.is_file():
        raise PipelineError("camera pipeline library is not a regular file")
    return source


def verify_pipelines(runtime: Path) -> None:
    gst = ctypes.CDLL(str(runtime_library(runtime, "libgstreamer-1.0.so.0")))
    gobject = ctypes.CDLL(str(runtime_library(runtime, "libgobject-2.0.so.0")))
    glib = ctypes.CDLL(str(runtime_library(runtime, "libglib-2.0.so.0")))
    gst.gst_init.argtypes = [ctypes.c_void_p, ctypes.c_void_p]
    gst.gst_init.restype = None
    gst.gst_parse_launch.argtypes = [ctypes.c_char_p, ctypes.POINTER(ctypes.POINTER(_GError))]
    gst.gst_parse_launch.restype = ctypes.c_void_p
    gobject.g_object_unref.argtypes = [ctypes.c_void_p]
    gobject.g_object_unref.restype = None
    glib.g_error_free.argtypes = [ctypes.POINTER(_GError)]
    glib.g_error_free.restype = None
    gst.gst_init(None, None)
    for name, description in PIPELINES.items():
        error = ctypes.POINTER(_GError)()
        pipeline = gst.gst_parse_launch(description.encode("ascii"), ctypes.byref(error))
        try:
            # GStreamer can return a partial pipeline while also setting GError.
            if error:
                message = (error.contents.message or b"unknown parser error").decode("utf-8", "replace")
                raise PipelineError(f"packaged {name} camera pipeline is incomplete: {message}")
            if not pipeline:
                raise PipelineError(f"packaged {name} camera pipeline is unavailable")
        finally:
            if pipeline:
                gobject.g_object_unref(pipeline)
            if error:
                glib.g_error_free(error)
    # Pipelines stay in NULL state. Only native desktop qualification starts capture.


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--runtime", required=True, type=Path)
    args = parser.parse_args()
    try:
        verify_pipelines(args.runtime)
    except (PipelineError, OSError) as error:
        print(f"ERROR: {error}", file=sys.stderr)
        return 1
    print("Packaged raw and MJPEG camera pipelines constructed successfully")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
