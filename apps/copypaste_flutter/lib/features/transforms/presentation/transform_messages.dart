import '../../../l10n/l10n.dart';
import '../models/transform_models.dart';

/// User-facing names for categories, failures and warnings. The runtime
/// sends stable codes only; every code family has a message here, and an
/// unknown code falls back to a generic one.
extension TransformMessages on AppLocalizations {
  String transformCategory(TransformCategory category) => switch (category) {
    TransformCategory.structure => transformsCategoryStructure,
    TransformCategory.encode => transformsCategoryEncode,
    TransformCategory.caseChange => transformsCategoryCase,
    TransformCategory.hash => transformsCategoryHash,
    TransformCategory.lines => transformsCategoryLines,
    TransformCategory.clean => transformsCategoryClean,
    TransformCategory.url => transformsCategoryUrl,
    TransformCategory.markup => transformsCategoryMarkup,
    TransformCategory.dev => transformsCategoryDev,
    TransformCategory.other => transformsCategoryOther,
  };

  String transformIssue(TransformIssue issue) {
    final message = _issueText(issue.code);
    final line = issue.line;
    final column = issue.column;
    if (line != null && column != null) {
      return transformsErrorAt(message, line, column);
    }
    return message;
  }

  String _issueText(String code) {
    switch (code) {
      case 'binary_data':
        return transformsErrorBinary;
      case 'input_too_large' || 'output_too_large':
        return transformsErrorTooLarge;
      case 'unknown_transform' || 'unknown_modifier' || 'unsupported_version':
        return transformsNeedsNewerVersion;
      case 'invalid_param':
        return transformsErrorInvalidParam;
      case 'cancelled':
        return transformsErrorCancelled;
      case 'budget_exceeded' || 'regex.too_expensive':
        return transformsErrorTooExpensive;
      case 'regex.invalid_pattern' || 'find.empty_pattern':
        return transformsErrorPattern;
      case 'timestamp.unit_required':
        return transformsErrorTimestampUnit;
      case 'xml.external_entity':
        return transformsErrorExternalEntity;
      case 'jwt.encrypted':
        return transformsErrorJwtEncrypted;
    }
    return switch (code.split('.').first) {
      'json' => transformsErrorJson,
      'yaml' => transformsErrorYaml,
      'xml' => transformsErrorXml,
      'csv' => transformsErrorCsv,
      'base64' => transformsErrorBase64,
      'hex' => transformsErrorHex,
      'escape' => transformsErrorEscape,
      'url' => transformsErrorUrl,
      'jwt' => transformsErrorJwt,
      'timestamp' || 'date' => transformsErrorDate,
      _ => transformsErrorGeneric,
    };
  }

  String transformWarning(TransformWarning warning) {
    final message = switch (warning.code) {
      'url.signed_unchanged' => transformsWarningSignedLink,
      'jwt.expired' => transformsWarningExpired,
      'jwt.not_yet_valid' => transformsWarningNotYetValid,
      'jwt.signature_not_verified' => transformsWarningSignatureNotVerified,
      'json.duplicate_key' ||
      'yaml.duplicate_key' => transformsWarningDuplicateKeys,
      'json.trailing_comma' || 'json.comment' => transformsWarningLenientJson,
      'json.double_encoded' => transformsWarningDoubleEncoded,
      'json.invalid_escape' ||
      'escape.unknown' ||
      'escape.invalid' => transformsWarningEscapes,
      'date.local_zone_assumed' => transformsWarningLocalZone,
      'extract.no_matches' => transformsWarningNoMatches,
      _ => transformsWarningGeneric,
    };
    final detail = warning.detail;
    return detail == null ? message : '$message ($detail)';
  }
}
