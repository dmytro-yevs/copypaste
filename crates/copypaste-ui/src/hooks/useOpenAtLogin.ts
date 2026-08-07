/**
 * Launch at login is a *system* setting with two editors — this screen and the
 * native tray menu — so it is a query rather than component state, and the
 * mutation writes back what the system reported rather than what was asked.
 */
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import { getOpenAtLogin, setOpenAtLogin } from "@/lib/ipc";

export const OPEN_AT_LOGIN_KEY = ["open-at-login"] as const;

export function useOpenAtLogin() {
  return useQuery({
    queryKey: OPEN_AT_LOGIN_KEY,
    queryFn: getOpenAtLogin,
    retry: false,
    staleTime: Infinity,
  });
}

export function useSetOpenAtLogin() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: setOpenAtLogin,
    onSuccess: (settled) => qc.setQueryData(OPEN_AT_LOGIN_KEY, settled),
  });
}
