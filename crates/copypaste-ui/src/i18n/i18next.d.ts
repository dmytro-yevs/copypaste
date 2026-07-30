import type { Catalogue } from "@/i18n/en";

declare module "i18next" {
  interface CustomTypeOptions {
    defaultNS: "app";
    resources: { app: Catalogue };
  }
}
