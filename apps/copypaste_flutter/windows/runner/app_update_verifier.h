#ifndef RUNNER_APP_UPDATE_VERIFIER_H_
#define RUNNER_APP_UPDATE_VERIFIER_H_

#include <string>

bool VerifyCopyPasteInstaller(const std::wstring& installer_path,
                              const std::string& expected_sha256);

bool CurrentCopyPasteExecutableIsSigned();

#endif  // RUNNER_APP_UPDATE_VERIFIER_H_
