import { CircleAlert, LoaderCircle } from "lucide-react";
import type { ReactNode } from "react";
import { Button } from "./shared/ui";

export function PageIntro({
  eyebrow,
  title,
  detail,
  action,
  onAction,
}: {
  eyebrow: string;
  title: string;
  detail: string;
  action?: string;
  onAction?: () => void;
}) {
  return (
    <div className="flex flex-col justify-between gap-4 sm:flex-row sm:items-end">
      <div>
        <span className="text-xs font-semibold uppercase tracking-wide text-primary">
          {eyebrow}
        </span>
        <h2 className="mt-1 text-2xl font-semibold">{title}</h2>
        <p className="mt-1 max-w-3xl text-sm text-muted-foreground">{detail}</p>
      </div>
      {action && <Button onClick={onAction}>{action}</Button>}
    </div>
  );
}

export function SurfaceHeading({
  label,
  title,
  trailing,
}: {
  label: string;
  title: string;
  trailing?: ReactNode;
}) {
  return (
    <div className="flex items-start justify-between gap-4">
      <div>
        <span className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
          {label}
        </span>
        <h3 className="mt-1 text-base font-semibold">{title}</h3>
      </div>
      {trailing}
    </div>
  );
}

export function ErrorNotice({ error }: { error: unknown }) {
  if (!error) return null;
  return (
    <div
      className="flex items-start gap-2 rounded-md border border-border bg-muted p-3 text-sm unavailable"
      role="alert"
    >
      <CircleAlert size={15} />
      <span>
        {error instanceof Error
          ? error.message
          : "owner projection unavailable"}
      </span>
    </div>
  );
}

export function QueryNotice({
  isPending,
  error,
  empty,
}: {
  isPending: boolean;
  error: unknown;
  empty: string;
}) {
  if (isPending)
    return (
      <div className="flex items-start gap-2 rounded-md border border-border bg-muted p-3 text-sm loading">
        <LoaderCircle className="animate-spin" size={15} /> 正在读取 owner
        projection...
      </div>
    );
  if (error) return <ErrorNotice error={error} />;
  return (
    <div className="flex items-start gap-2 rounded-md border border-border bg-muted p-3 text-sm empty">
      {empty}
    </div>
  );
}
