import { CircleAlert, CircleCheck, LoaderCircle } from "lucide-react";
import type { ComponentProps, ReactNode } from "react";
import {
  Badge,
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "../shared/ui";
import { statusLabel } from "./display";

export function WorkbenchPanel({
  children,
  className = "",
  ...props
}: ComponentProps<typeof Card>) {
  return (
    <Card className={`shadow-sm ${className}`} {...props}>
      {children}
    </Card>
  );
}

export function WorkbenchPanelHeader({
  icon,
  label,
  title,
  detail,
  trailing,
}: {
  icon?: ReactNode;
  label: string;
  title: string;
  detail?: string;
  trailing?: ReactNode;
}) {
  return (
    <CardHeader className="flex flex-row items-start justify-between gap-4">
      <div className="min-w-0">
        <div className="flex items-center gap-2 text-xs font-semibold tracking-wide text-muted-foreground">
          {icon}
          <span>{label}</span>
        </div>
        <CardTitle className="mt-1 truncate">{title}</CardTitle>
        {detail && <CardDescription className="mt-1">{detail}</CardDescription>}
      </div>
      {trailing}
    </CardHeader>
  );
}

export function WorkbenchNotice({
  children,
  tone = "muted",
}: {
  children: ReactNode;
  tone?: "muted" | "success" | "warning" | "danger";
}) {
  const tones = {
    muted: "border-border bg-muted text-muted-foreground",
    success: "border-success/30 bg-success/10 text-success",
    warning: "border-warning/30 bg-warning/10 text-warning-foreground",
    danger: "border-destructive/30 bg-destructive/10 text-destructive",
  } as const;
  const Icon =
    tone === "success" ? CircleCheck : tone === "muted" ? null : CircleAlert;
  return (
    <div
      className={`flex items-start gap-2 rounded-md border p-3 text-sm ${tones[tone]}`}
      role={tone === "danger" ? "alert" : "status"}
    >
      {Icon && <Icon aria-hidden="true" className="mt-0.5 size-4 shrink-0" />}
      <span>{children}</span>
    </div>
  );
}

export function WorkbenchQueryNotice({
  isPending,
  error,
}: {
  isPending: boolean;
  error: unknown;
}) {
  if (isPending)
    return (
      <WorkbenchNotice>
        <span className="inline-flex items-center gap-2">
          <LoaderCircle aria-hidden="true" className="size-4 animate-spin" />{" "}
          正在读取项目数据…
        </span>
      </WorkbenchNotice>
    );
  if (error)
    return (
      <WorkbenchNotice tone="danger">
        {error instanceof Error
          ? error.message
          : "项目数据暂时不可用，请重试。"}
      </WorkbenchNotice>
    );
  return null;
}

export function WorkbenchStatus({
  value,
}: {
  value: string | null | undefined;
}) {
  const tone =
    value === "failed" || value === "stale"
      ? "warning"
      : value === "succeeded" || value === "ready"
        ? "success"
        : "secondary";
  return <Badge variant={tone}>{statusLabel(value)}</Badge>;
}

export { CardContent };
