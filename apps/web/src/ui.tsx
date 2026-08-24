import { CircleAlert, LoaderCircle } from "lucide-react";
import type { ReactNode } from "react";

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
    <div className="page-intro">
      <div>
        <span className="micro-label accent">{eyebrow}</span>
        <h2>{title}</h2>
        <p>{detail}</p>
      </div>
      {action && (
        <button className="primary-button" onClick={onAction}>
          {action}
        </button>
      )}
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
    <div className="surface-heading">
      <div>
        <span className="micro-label">{label}</span>
        <h3>{title}</h3>
      </div>
      {trailing}
    </div>
  );
}

export function ErrorNotice({ error }: { error: unknown }) {
  if (!error) return null;
  return (
    <div className="data-notice unavailable" role="alert">
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
      <div className="data-notice loading">
        <LoaderCircle className="spin" size={15} /> 正在读取 owner projection...
      </div>
    );
  if (error) return <ErrorNotice error={error} />;
  return <div className="data-notice empty">{empty}</div>;
}
