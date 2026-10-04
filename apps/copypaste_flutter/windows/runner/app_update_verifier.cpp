#include "app_update_verifier.h"

#include <windows.h>

#include <bcrypt.h>
#include <softpub.h>
#include <wincrypt.h>
#include <wintrust.h>

#include <array>
#include <iomanip>
#include <optional>
#include <sstream>
#include <utility>
#include <vector>

namespace {

std::optional<std::string> Sha256File(const std::wstring& path) {
  BCRYPT_ALG_HANDLE algorithm = nullptr;
  BCRYPT_HASH_HANDLE hash = nullptr;
  DWORD object_size = 0;
  DWORD hash_size = 0;
  DWORD received = 0;
  if (!BCRYPT_SUCCESS(::BCryptOpenAlgorithmProvider(
          &algorithm, BCRYPT_SHA256_ALGORITHM, nullptr, 0)) ||
      !BCRYPT_SUCCESS(
          ::BCryptGetProperty(algorithm, BCRYPT_OBJECT_LENGTH,
                              reinterpret_cast<PUCHAR>(&object_size),
                              sizeof(object_size), &received, 0)) ||
      !BCRYPT_SUCCESS(::BCryptGetProperty(algorithm, BCRYPT_HASH_LENGTH,
                                          reinterpret_cast<PUCHAR>(&hash_size),
                                          sizeof(hash_size), &received, 0))) {
    if (algorithm != nullptr) ::BCryptCloseAlgorithmProvider(algorithm, 0);
    return std::nullopt;
  }
  std::vector<UCHAR> object(object_size);
  std::vector<UCHAR> digest(hash_size);
  if (!BCRYPT_SUCCESS(::BCryptCreateHash(algorithm, &hash, object.data(),
                                         object_size, nullptr, 0, 0))) {
    ::BCryptCloseAlgorithmProvider(algorithm, 0);
    return std::nullopt;
  }
  const HANDLE file = ::CreateFileW(
      path.c_str(), GENERIC_READ, FILE_SHARE_READ, nullptr, OPEN_EXISTING,
      FILE_ATTRIBUTE_NORMAL | FILE_FLAG_SEQUENTIAL_SCAN, nullptr);
  if (file == INVALID_HANDLE_VALUE) {
    ::BCryptDestroyHash(hash);
    ::BCryptCloseAlgorithmProvider(algorithm, 0);
    return std::nullopt;
  }
  std::array<UCHAR, 64 * 1024> buffer{};
  bool valid = true;
  for (;;) {
    DWORD read = 0;
    if (!::ReadFile(file, buffer.data(), static_cast<DWORD>(buffer.size()),
                    &read, nullptr)) {
      valid = false;
      break;
    }
    if (read == 0) break;
    if (!BCRYPT_SUCCESS(::BCryptHashData(hash, buffer.data(), read, 0))) {
      valid = false;
      break;
    }
  }
  ::CloseHandle(file);
  if (valid) {
    valid =
        BCRYPT_SUCCESS(::BCryptFinishHash(hash, digest.data(), hash_size, 0));
  }
  ::BCryptDestroyHash(hash);
  ::BCryptCloseAlgorithmProvider(algorithm, 0);
  if (!valid) return std::nullopt;
  std::ostringstream output;
  output << std::hex << std::setfill('0');
  for (const auto byte : digest)
    output << std::setw(2) << static_cast<int>(byte);
  return output.str();
}

std::optional<std::vector<BYTE>> SignerThumbprint(const std::wstring& path) {
  DWORD encoding = 0;
  DWORD content = 0;
  DWORD format = 0;
  HCERTSTORE store = nullptr;
  HCRYPTMSG message = nullptr;
  if (!::CryptQueryObject(CERT_QUERY_OBJECT_FILE, path.c_str(),
                          CERT_QUERY_CONTENT_FLAG_PKCS7_SIGNED_EMBED,
                          CERT_QUERY_FORMAT_FLAG_BINARY, 0, &encoding, &content,
                          &format, &store, &message, nullptr)) {
    return std::nullopt;
  }
  DWORD signer_size = 0;
  if (!::CryptMsgGetParam(message, CMSG_SIGNER_INFO_PARAM, 0, nullptr,
                          &signer_size) ||
      signer_size == 0 || signer_size > 1024 * 1024) {
    ::CryptMsgClose(message);
    ::CertCloseStore(store, 0);
    return std::nullopt;
  }
  std::vector<BYTE> signer_bytes(signer_size);
  if (!::CryptMsgGetParam(message, CMSG_SIGNER_INFO_PARAM, 0,
                          signer_bytes.data(), &signer_size)) {
    ::CryptMsgClose(message);
    ::CertCloseStore(store, 0);
    return std::nullopt;
  }
  const auto* signer =
      reinterpret_cast<const CMSG_SIGNER_INFO*>(signer_bytes.data());
  CERT_INFO certificate_info{};
  certificate_info.Issuer = signer->Issuer;
  certificate_info.SerialNumber = signer->SerialNumber;
  PCCERT_CONTEXT certificate = ::CertFindCertificateInStore(
      store, X509_ASN_ENCODING | PKCS_7_ASN_ENCODING, 0, CERT_FIND_SUBJECT_CERT,
      &certificate_info, nullptr);
  std::optional<std::vector<BYTE>> thumbprint;
  if (certificate != nullptr) {
    DWORD thumbprint_size = 0;
    if (::CertGetCertificateContextProperty(
            certificate, CERT_SHA256_HASH_PROP_ID, nullptr, &thumbprint_size) &&
        thumbprint_size > 0 && thumbprint_size <= 128) {
      std::vector<BYTE> value(thumbprint_size);
      if (::CertGetCertificateContextProperty(certificate,
                                              CERT_SHA256_HASH_PROP_ID,
                                              value.data(), &thumbprint_size)) {
        value.resize(thumbprint_size);
        thumbprint = std::move(value);
      }
    }
    ::CertFreeCertificateContext(certificate);
  }
  ::CryptMsgClose(message);
  ::CertCloseStore(store, 0);
  return thumbprint;
}

bool HasValidAuthenticodeDigest(const std::wstring& path) {
  WINTRUST_FILE_INFO file_info{};
  file_info.cbStruct = sizeof(file_info);
  file_info.pcwszFilePath = path.c_str();

  WINTRUST_DATA trust_data{};
  trust_data.cbStruct = sizeof(trust_data);
  trust_data.dwUIChoice = WTD_UI_NONE;
  trust_data.fdwRevocationChecks = WTD_REVOKE_NONE;
  trust_data.dwUnionChoice = WTD_CHOICE_FILE;
  trust_data.pFile = &file_info;
  trust_data.dwStateAction = WTD_STATEACTION_VERIFY;
  trust_data.dwProvFlags = WTD_CACHE_ONLY_URL_RETRIEVAL;
  GUID policy = WINTRUST_ACTION_GENERIC_VERIFY_V2;
  const LONG status = ::WinVerifyTrust(nullptr, &policy, &trust_data);
  trust_data.dwStateAction = WTD_STATEACTION_CLOSE;
  ::WinVerifyTrust(nullptr, &policy, &trust_data);
  return status == ERROR_SUCCESS || status == CERT_E_UNTRUSTEDROOT ||
         status == CERT_E_CHAINING;
}

std::optional<std::wstring> CurrentExecutable() {
  std::vector<wchar_t> path(32768);
  const DWORD length = ::GetModuleFileNameW(nullptr, path.data(),
                                            static_cast<DWORD>(path.size()));
  if (length == 0 || length >= path.size()) return std::nullopt;
  return std::wstring(path.data(), length);
}

}  // namespace

bool VerifyCopyPasteInstaller(const std::wstring& installer_path,
                              const std::string& expected_sha256) {
  if (expected_sha256.size() != 64) return false;
  const auto current_executable = CurrentExecutable();
  const auto actual_sha256 = Sha256File(installer_path);
  const auto current_signer = current_executable.has_value()
                                  ? SignerThumbprint(*current_executable)
                                  : std::nullopt;
  const auto installer_signer = SignerThumbprint(installer_path);
  return actual_sha256 == expected_sha256 && current_signer.has_value() &&
         installer_signer.has_value() && *current_signer == *installer_signer &&
         HasValidAuthenticodeDigest(installer_path);
}

bool CurrentCopyPasteExecutableIsSigned() {
  const auto current = CurrentExecutable();
  return current.has_value() && SignerThumbprint(*current).has_value();
}
