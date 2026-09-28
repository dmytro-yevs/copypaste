import { useEffect, useId, useRef } from "react";

import { SettingsRow } from "@/components/shared";
import { StateView } from "@/components/shared/StateView";
import { Button, Input, Select, Slider, Switch } from "@/components/ui";
import { Icon } from "@/components/ui/icon";
import { SettingsGroupSurface } from "@/features/settings/components/SettingsGroupSurface";
import type { SettingsField, SettingsGroupSchema } from "@/features/settings/model/settingsFieldSchema";
import { useTranslation } from "@/i18n";
import styles from "./SettingsSchemaRenderer.module.css";

export function SettingsSchemaRenderer({ groups }: { readonly groups: readonly SettingsGroupSchema[] }) {
  return <>{groups.map((group) => <SettingsSchemaGroup key={group.id} group={group} />)}</>;
}

function SettingsSchemaGroup({ group }: { readonly group: SettingsGroupSchema }) {
  const disclosureRef = useRef<HTMLDetailsElement>(null);
  useEffect(() => { if (group.revealKey !== undefined && disclosureRef.current) disclosureRef.current.open = true; }, [group.revealKey]);
  const content = group.fields.filter((field) => field.visible !== false).map((field) => (
    <SettingsSchemaField key={field.definition.id} field={field} />
  ));
  if (content.length === 0) return null;
  if (group.disclosure) {
    return <details ref={disclosureRef} className={styles.disclosure} data-settings-search-target={`section:${group.title ?? ""}`}><summary className={styles.disclosureTrigger}><span className={styles.disclosureCopy}><strong>{group.title}</strong>{group.description ? <span>{group.description}</span> : null}</span></summary><div className={styles.disclosureContent}>{content}</div></details>;
  }
  if (group.title) return <section data-settings-search-target={`section:${group.title}`} className={styles.section}><div className={styles.heading}><h2 className={styles.title}>{group.title}</h2>{group.description ? <p className={styles.description}>{group.description}</p> : null}</div><SettingsGroupSurface>{content}</SettingsGroupSurface></section>;
  if (group.surface === false) return <>{content}</>;
  return <SettingsGroupSurface>{content}</SettingsGroupSurface>;
}

export function SettingsSchemaField({ field }: { readonly field: SettingsField }) {
  const { t } = useTranslation();
  const errorId = useId();
  const noteId = useId();
  if (field.visible === false) return null;
  const title = t(field.definition.title as never);
  const help = field.help ?? (field.definition.description ? t(field.definition.description as never) : undefined);
  const invalid = field.kind === "choice" && field.validation !== undefined &&
    (Number(field.value) < field.validation.min ||
      (field.validation.max !== undefined && Number(field.value) > field.validation.max));
  const describedBy = field.note === undefined ? undefined : noteId;
  let control;
  switch (field.kind) {
    case "boolean": control = <Switch id={field.controlId} aria-label={title} aria-describedby={describedBy} checked={field.value} disabled={field.disabled} aria-busy={field.busy || undefined} onCheckedChange={field.onChange} />; break;
    case "choice": control = field.presentation === "segmented" || field.presentation === "cards" ? <div role="group" aria-label={title} className={field.controlClassName}>{field.options.map((option) => <Button key={option.value} type="button" variant={field.presentation === "cards" ? "ghost" : field.value === option.value ? "secondary" : "ghost"} size={field.presentation === "cards" ? "md" : "sm"} className={field.optionClassName} data-product-theme={field.presentation === "cards" ? option.value : undefined} aria-label={option.label} aria-describedby={describedBy} aria-pressed={field.value === option.value} disabled={field.disabled} onClick={(event) => field.onChange(option.value, event.currentTarget)}>{field.renderOption?.(option) ?? option.label}</Button>)}</div> : <Select size="sm" aria-label={title} aria-describedby={describedBy} value={field.value} disabled={field.disabled} aria-busy={field.busy || undefined} aria-invalid={invalid || undefined} aria-errormessage={invalid ? errorId : undefined} leadingIcon={field.leadingIcon} items={field.options} onValueChange={(value) => field.onChange(value)} />; break;
    case "multi-choice": control = <Select mode="multiple" aria-label={title} aria-describedby={describedBy} values={field.value} items={field.options} disabled={field.disabled} allLabel={title} onValuesChange={field.onChange} />; break;
    case "text": control = <Input size="sm" aria-label={title} aria-describedby={describedBy} value={field.value} disabled={field.disabled} placeholder={field.placeholder} onChange={(event) => field.onChange(event.currentTarget.value)} />; break;
    case "number": control = <><output>{field.displayValue ?? field.value.toLocaleString()}</output><Slider aria-label={title} aria-describedby={describedBy} value={[field.value]} min={field.min} max={field.max} step={field.step ?? 1} disabled={field.disabled} onValueChange={([value]) => { if (value !== undefined) field.onChange(value); }} /></>; break;
    case "readonly": case "status": control = field.value; break;
    case "action": control = <>{field.href ? <Button asChild size="sm" variant={field.variant ?? "secondary"} tone={field.tone}><a href={field.href} target="_blank" rel="noreferrer" aria-describedby={describedBy}>{field.icon ? <Icon name={field.icon} aria-hidden="true" /> : null}{field.label}</a></Button> : <Button type="button" size="sm" variant={field.variant ?? "secondary"} tone={field.tone} disabled={field.disabled} aria-describedby={describedBy} aria-busy={field.busy || undefined} onClick={field.onAction}>{field.icon ? <Icon name={field.icon} aria-hidden="true" /> : null}{field.label}</Button>}{field.extraActions}</>; break;
    case "custom": control = field.content; break;
  }
  const note = invalid && field.kind === "choice" ? <><StateView id={errorId} mode="error" placement="control" title={field.validation?.message} />{field.note === undefined ? null : <span id={noteId}>{field.note}</span>}</> : field.note === undefined ? undefined : <span id={noteId}>{field.note}</span>;
  if (field.kind === "custom" && field.rowless) return <div data-settings-search-target={`row:${title}`} aria-busy={field.busy || undefined}>{field.content}</div>;
  if (field.kind === "choice" && field.presentation === "cards") return <div data-settings-search-target={`row:${title}`}><div className={field.titleClassName}>{title}</div>{control}</div>;
  return <SettingsRow title={title} help={help} badge={field.badge} note={note}>{control}</SettingsRow>;
}
