#ifndef RUNNER_APP_UPDATE_HELPER_H_
#define RUNNER_APP_UPDATE_HELPER_H_

#include <string>
#include <vector>

// Returns -1 for a normal app launch, otherwise the helper process exit code.
int RunAppUpdateHelperIfRequested(const std::vector<std::string>& arguments);

#endif  // RUNNER_APP_UPDATE_HELPER_H_
