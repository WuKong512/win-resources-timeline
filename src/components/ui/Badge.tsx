import type { HTMLAttributes } from "react";
import { cn } from "./utils";

export function Badge({ className, ...props }: HTMLAttributes<HTMLSpanElement>) {
  return (
    <span
      className={cn("inline-flex items-center rounded-full border border-primary/15 bg-accent px-2.5 py-0.5 text-[11px] font-semibold text-accent-foreground", className)}
      {...props}
    />
  );
}
