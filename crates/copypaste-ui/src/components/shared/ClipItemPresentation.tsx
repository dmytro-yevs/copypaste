import type { ComponentProps, ReactNode } from "react";

import { VisuallyHidden } from "@/components/ui";
import { ClipBodyPreview } from "./ClipBodyPreview";
import { SourceMeta } from "./SourceMeta";

interface ClipItemPresentationProps {
    preview: ComponentProps<typeof ClipBodyPreview>;
    metadata: ComponentProps<typeof SourceMeta>;
    bodyClassName: string;
    bodyAccessory?: ReactNode;
    afterBody?: ReactNode;
    hideMetadata?: boolean;
}

/** Shared clip content; each surface keeps its own hit areas and event lifecycle. */
export function ClipItemPresentation({
    preview,
    metadata,
    bodyClassName,
    bodyAccessory,
    afterBody,
    hideMetadata = false,
}: ClipItemPresentationProps) {
    const meta = <SourceMeta {...metadata} />;
    return <>
        <div className={bodyClassName}>
            <ClipBodyPreview {...preview} />
            {bodyAccessory}
        </div>
        {afterBody}
        {hideMetadata ? <VisuallyHidden asChild>{meta}</VisuallyHidden> : meta}
    </>;
}
