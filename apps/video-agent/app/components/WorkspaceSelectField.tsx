"use client";

import type { ReactNode, SelectHTMLAttributes } from "react";

type Props = Omit<SelectHTMLAttributes<HTMLSelectElement>, "children"> & {
  label: string;
  children: ReactNode;
  fieldClassName?: string;
};

export function WorkspaceSelectField({
  label,
  children,
  fieldClassName = "",
  className = "",
  ...selectProps
}: Props) {
  return (
    <label className={`workspaceSelectField${fieldClassName ? ` ${fieldClassName}` : ""}`}>
      <span className="workspaceSelectLabel">{label}</span>
      <span className="workspaceSelectControl">
        <select aria-label={label} className={className} {...selectProps}>{children}</select>
        <span aria-hidden="true" className="workspaceSelectChevron" />
      </span>
    </label>
  );
}
