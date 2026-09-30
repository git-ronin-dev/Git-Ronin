import { clsx } from "clsx";
import type { ButtonHTMLAttributes } from "react";

type Variant = "primary" | "secondary" | "ghost" | "danger" | "dangerGhost";

const variants: Record<Variant, string> = {
  primary: "bg-accent text-accent-fg hover:brightness-110",
  danger: "bg-danger text-accent-fg hover:brightness-110",
  secondary: "border border-line bg-raised text-fg hover:bg-hover",
  ghost: "text-fg-muted hover:bg-hover hover:text-fg",
  /** A destructive action that shouldn't draw the eye. */
  dangerGhost: "text-danger hover:bg-hover",
};

export interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: Variant;
}

export function Button({
  variant = "secondary",
  type = "button",
  className,
  ...props
}: ButtonProps) {
  return (
    <button
      type={type}
      data-primary={variant === "primary" ? "" : undefined}
      className={clsx(
        "inline-flex h-7 items-center gap-1.5 rounded-md px-3 font-medium transition-colors",
        "disabled:pointer-events-none disabled:opacity-40",
        variants[variant],
        className,
      )}
      {...props}
    />
  );
}
