import type { OnboardingPermissionId, OnboardingPermissions } from "@/generated/ipc";

let permissions: OnboardingPermissions = {
  platform: "android",
  notifications: { id: "notifications", status: "prompt", required: false },
  tile: { id: "tile", status: "prompt", required: false },
  clipboardStatus: "not_required",
};

export function previewPermissionSnapshot(): OnboardingPermissions {
  return permissions;
}

export function grantPreviewPermission(id: OnboardingPermissionId): OnboardingPermissions {
  permissions = {
    ...permissions,
    [id]: { ...permissions[id], status: "granted" },
  };
  return permissions;
}
