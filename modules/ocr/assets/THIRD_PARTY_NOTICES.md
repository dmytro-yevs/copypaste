# Third-party notices

This module includes these components only when its preparation scripts place
them in a package. The base CopyPaste installation does not include them.

- `paddle-ocr-rs` 0.6.1, Apache-2.0, <https://github.com/mg-chao/paddle-ocr-rs>.
  The vendored copy updates the ONNX Runtime binding to 2.0.0-rc.13 so the
  upstream process-exit environment release is used.
- ONNX Runtime 1.28, MIT, <https://github.com/microsoft/onnxruntime>
- PP-OCRv5 ONNX model weights from PaddlePaddle, Apache-2.0. Source URLs,
  immutable revisions, and SHA-256 values are recorded in `model-sources.json`.

The preparation scripts verify every model file before it can be packaged.
