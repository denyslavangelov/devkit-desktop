import { LoaderCircle } from "lucide-react";
import type { ButtonHTMLAttributes, ReactNode } from "react";

type Variant = "primary" | "secondary" | "danger";

type Props = ButtonHTMLAttributes<HTMLButtonElement> & {
  variant?: Variant;
  loading?: boolean;
  icon?: ReactNode;
  grow?: boolean;
};

export function Button({
  variant = "secondary",
  loading = false,
  icon,
  grow = false,
  className = "",
  children,
  disabled,
  ...rest
}: Props) {
  const classes = [
    variant,
    grow ? "grow" : "",
    loading ? "is-loading" : "",
    className,
  ]
    .filter(Boolean)
    .join(" ");

  return (
    <button className={classes} disabled={disabled || loading} {...rest}>
      {loading ? <LoaderCircle size={16} className="spin btn-spinner" /> : icon}
      <span className="btn-label">{children}</span>
    </button>
  );
}
