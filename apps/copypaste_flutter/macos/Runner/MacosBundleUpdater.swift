import CryptoKit
import Darwin
import Foundation
import Security

enum MacosBundleUpdateError: Error, Equatable {
  case invalidArguments
  case verificationFailed
  case packageInvalid
  case signatureInvalid
  case downgradeRefused
  case translocated
  case unwritable
  case replacementBlocked
  case failed

  var code: String {
    switch self {
    case .invalidArguments: return "invalid_arguments"
    case .verificationFailed: return "verification_failed"
    case .packageInvalid: return "package_invalid"
    case .signatureInvalid: return "signature_invalid"
    case .downgradeRefused: return "downgrade_refused"
    case .translocated: return "install_location_translocated"
    case .unwritable: return "install_location_unwritable"
    case .replacementBlocked: return "replacement_blocked"
    case .failed: return "update_failed"
    }
  }

  var reason: String {
    switch self {
    case .translocated:
      return "Move CopyPaste to the Applications folder and open it from there to update."
    case .unwritable:
      return "CopyPaste cannot replace itself in its current folder. Use the release page to update."
    default:
      return "CopyPaste could not install the update."
    }
  }
}

/// Replaces a non-Homebrew bundle with the app from a release disk image whose
/// updater signature the Dart side has already verified against the pinned key.
struct MacosBundleUpdater {
  private static let imageApplicationName = "CopyPaste.app"
  private static let hdiutil = URL(fileURLWithPath: "/usr/bin/hdiutil")
  private static let ditto = URL(fileURLWithPath: "/usr/bin/ditto")

  static var architecture: String {
    #if arch(arm64)
      return "arm64"
    #else
      return "x86_64"
    #endif
  }

  let installedURL: URL
  let bundleIdentifier: String
  let currentVersion: String

  static func current() -> MacosBundleUpdater? {
    guard
      let identifier = Bundle.main.bundleIdentifier,
      let version = Bundle.main.object(
        forInfoDictionaryKey: "CFBundleShortVersionString"
      ) as? String
    else { return nil }
    return MacosBundleUpdater(
      installedURL: Bundle.main.bundleURL,
      bundleIdentifier: identifier,
      currentVersion: version
    )
  }

  /// Gatekeeper runs quarantined downloads from a randomized read-only path,
  /// and a bundle can only be swapped where this account may rename it.
  func locationProblem() -> MacosBundleUpdateError? {
    let resolved = installedURL.resolvingSymlinksInPath()
    if resolved.pathComponents.contains("AppTranslocation") {
      return .translocated
    }
    guard
      let values = try? resolved.resourceValues(forKeys: [.volumeIsReadOnlyKey]),
      let readOnly = values.volumeIsReadOnly
    else { return .unwritable }
    if readOnly { return .translocated }
    let parent = resolved.deletingLastPathComponent()
    guard access(parent.path, W_OK) == 0, access(resolved.path, W_OK) == 0 else {
      return .unwritable
    }
    return nil
  }

  func install(image: URL, sha256: String, version: String) throws {
    guard
      sha256.range(of: "^[a-f0-9]{64}$", options: .regularExpression) != nil,
      image.isFileURL,
      image.lastPathComponent == "CopyPaste-v\(version)-macos-\(Self.architecture).dmg",
      image.deletingLastPathComponent().lastPathComponent == "copypaste-updates"
    else { throw MacosBundleUpdateError.invalidArguments }
    guard Self.isNewer(version, than: currentVersion) else {
      throw MacosBundleUpdateError.downgradeRefused
    }
    if let problem = locationProblem() { throw problem }
    guard Self.isRegularFile(image), (try? Self.sha256Hex(of: image)) == sha256 else {
      throw MacosBundleUpdateError.verificationFailed
    }
    let identity = try SigningIdentity.running()
    let installed = installedURL.resolvingSymlinksInPath()

    let replacement: URL
    do {
      replacement = try FileManager.default.url(
        for: .itemReplacementDirectory,
        in: .userDomainMask,
        appropriateFor: installed,
        create: true
      )
    } catch {
      throw MacosBundleUpdateError.unwritable
    }
    defer { try? FileManager.default.removeItem(at: replacement) }
    let staged = replacement.appendingPathComponent(installed.lastPathComponent)

    try Self.withMountedImage(image) { mount in
      let candidate = mount.appendingPathComponent(Self.imageApplicationName)
      guard Self.isDirectory(candidate) else {
        throw MacosBundleUpdateError.packageInvalid
      }
      guard MacosAppUpdateChannel.run(
        Self.ditto,
        arguments: [candidate.path, staged.path],
        timeout: 300
      ) else { throw MacosBundleUpdateError.failed }
    }

    // Validate the private staged copy, which is exactly what gets installed.
    try validate(staged, version: version, identity: identity)
    guard Self.device(of: staged) == Self.device(of: installed) else {
      throw MacosBundleUpdateError.unwritable
    }
    if renamex_np(staged.path, installed.path, UInt32(RENAME_SWAP)) != 0 {
      let failure = errno
      throw failure == EPERM || failure == EACCES
        ? MacosBundleUpdateError.replacementBlocked
        : MacosBundleUpdateError.failed
    }
  }

  private func validate(
    _ bundle: URL,
    version: String,
    identity: SigningIdentity
  ) throws {
    guard
      let info = NSDictionary(
        contentsOf: bundle.appendingPathComponent("Contents/Info.plist")
      ),
      info["CFBundleIdentifier"] as? String == bundleIdentifier,
      info["CFBundleShortVersionString"] as? String == version
    else { throw MacosBundleUpdateError.packageInvalid }

    var code: SecStaticCode?
    guard SecStaticCodeCreateWithPath(bundle as CFURL, [], &code) == errSecSuccess,
          let code else { throw MacosBundleUpdateError.signatureInvalid }
    // Equivalent to `codesign --verify --deep --strict` for every architecture.
    let flags = SecCSFlags(
      rawValue: kSecCSCheckAllArchitectures | kSecCSStrictValidate | kSecCSCheckNestedCode
    )
    // Ad-hoc designated requirements are a per-build cdhash, so they can only
    // be compared through the signing identifier and team instead.
    let requirement = identity.adhoc ? nil : identity.designatedRequirement
    guard SecStaticCodeCheckValidityWithErrors(code, flags, requirement, nil) == errSecSuccess,
          let candidate = SigningIdentity(code: code),
          candidate.identifier == identity.identifier,
          candidate.teamIdentifier == identity.teamIdentifier
    else { throw MacosBundleUpdateError.signatureInvalid }
  }

  private static func withMountedImage(
    _ image: URL,
    _ body: (URL) throws -> Void
  ) throws {
    let root = FileManager.default.temporaryDirectory
      .appendingPathComponent("copypaste-update-\(UUID().uuidString)")
    do {
      try FileManager.default.createDirectory(
        at: root,
        withIntermediateDirectories: false,
        attributes: [.posixPermissions: 0o700]
      )
    } catch {
      throw MacosBundleUpdateError.failed
    }
    // rmdir never descends into a volume that failed to detach.
    defer { rmdir(root.path) }
    let (attached, output) = MacosAppUpdateChannel.runCapturing(
      hdiutil,
      arguments: [
        "attach", "-nobrowse", "-readonly", "-noautoopen",
        "-mountrandom", root.path, "-plist", image.path,
      ],
      timeout: 120
    )
    guard attached else { throw MacosBundleUpdateError.packageInvalid }
    let mounts = mountPoints(in: output)
    defer { mounts.forEach(detach) }
    guard mounts.count == 1, let mount = mounts.first,
          mount.deletingLastPathComponent().resolvingSymlinksInPath().path
            == root.resolvingSymlinksInPath().path
    else {
      throw MacosBundleUpdateError.packageInvalid
    }
    try body(mount)
  }

  private static func mountPoints(in plist: String) -> [URL] {
    guard
      let data = plist.data(using: .utf8),
      let root = try? PropertyListSerialization.propertyList(
        from: data, format: nil
      ) as? [String: Any],
      let entities = root["system-entities"] as? [[String: Any]]
    else { return [] }
    return entities.compactMap { entity in
      (entity["mount-point"] as? String).map { URL(fileURLWithPath: $0) }
    }
  }

  private static func detach(_ mount: URL) {
    if !MacosAppUpdateChannel.run(hdiutil, arguments: ["detach", mount.path], timeout: 60) {
      _ = MacosAppUpdateChannel.run(
        hdiutil, arguments: ["detach", "-force", mount.path], timeout: 60
      )
    }
    rmdir(mount.path)
  }

  private static func isRegularFile(_ url: URL) -> Bool {
    let values = try? url.resourceValues(forKeys: [.isRegularFileKey, .isSymbolicLinkKey])
    return values?.isRegularFile == true && values?.isSymbolicLink != true
  }

  private static func isDirectory(_ url: URL) -> Bool {
    let values = try? url.resourceValues(forKeys: [.isDirectoryKey, .isSymbolicLinkKey])
    return values?.isDirectory == true && values?.isSymbolicLink != true
  }

  private static func device(of url: URL) -> dev_t? {
    var status = stat()
    return lstat(url.path, &status) == 0 ? status.st_dev : nil
  }

  private static func sha256Hex(of url: URL) throws -> String {
    let handle = try FileHandle(forReadingFrom: url)
    defer { try? handle.close() }
    var hasher = SHA256()
    while let chunk = try handle.read(upToCount: 1024 * 1024), !chunk.isEmpty {
      hasher.update(data: chunk)
    }
    return hasher.finalize().map { String(format: "%02x", $0) }.joined()
  }

  /// Semantic-version precedence; build metadata does not order releases.
  static func isNewer(_ candidate: String, than current: String) -> Bool {
    guard let next = SemanticVersion(candidate),
          let installed = SemanticVersion(current) else { return false }
    return installed < next
  }
}

private struct SigningIdentity {
  let identifier: String
  let teamIdentifier: String?
  let adhoc: Bool
  var designatedRequirement: SecRequirement?

  init?(code: SecStaticCode) {
    var raw: CFDictionary?
    guard
      SecCodeCopySigningInformation(
        code, SecCSFlags(rawValue: kSecCSSigningInformation), &raw
      ) == errSecSuccess,
      let info = raw as? [String: Any],
      let identifier = info[kSecCodeInfoIdentifier as String] as? String
    else { return nil }
    let flags = (info[kSecCodeInfoFlags as String] as? NSNumber)?.uint32Value ?? 0
    self.identifier = identifier
    teamIdentifier = info[kSecCodeInfoTeamIdentifier as String] as? String
    adhoc = flags & SecCodeSignatureFlags.adhoc.rawValue != 0
    var requirement: SecRequirement?
    if SecCodeCopyDesignatedRequirement(code, [], &requirement) == errSecSuccess {
      designatedRequirement = requirement
    }
  }

  static func running() throws -> SigningIdentity {
    var running: SecCode?
    var code: SecStaticCode?
    guard SecCodeCopySelf([], &running) == errSecSuccess, let running,
          SecCodeCopyStaticCode(running, [], &code) == errSecSuccess, let code,
          let identity = SigningIdentity(code: code),
          identity.adhoc || identity.designatedRequirement != nil
    else { throw MacosBundleUpdateError.signatureInvalid }
    return identity
  }
}

private struct SemanticVersion: Comparable {
  let core: [Int]
  let prerelease: [String]

  init?(_ value: String) {
    let withoutBuild = value.split(separator: "+", maxSplits: 1, omittingEmptySubsequences: false)[0]
    let parts = withoutBuild.split(separator: "-", maxSplits: 1, omittingEmptySubsequences: false)
    let fields = parts[0].split(separator: ".", omittingEmptySubsequences: false)
    let core = fields.compactMap { Int($0) }
    guard fields.count == 3, core.count == 3, core.allSatisfy({ $0 >= 0 }) else { return nil }
    self.core = core
    prerelease = parts.count == 2 ? parts[1].split(separator: ".").map(String.init) : []
  }

  static func < (lhs: SemanticVersion, rhs: SemanticVersion) -> Bool {
    if lhs.core != rhs.core { return lhs.core.lexicographicallyPrecedes(rhs.core) }
    if lhs.prerelease.isEmpty || rhs.prerelease.isEmpty {
      return !lhs.prerelease.isEmpty && rhs.prerelease.isEmpty
    }
    for (left, right) in zip(lhs.prerelease, rhs.prerelease) where left != right {
      switch (Int(left), Int(right)) {
      case let (l?, r?): return l < r
      case (.some, nil): return true
      case (nil, .some): return false
      default: return left < right
      }
    }
    return lhs.prerelease.count < rhs.prerelease.count
  }
}
