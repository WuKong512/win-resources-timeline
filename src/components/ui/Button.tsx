import type { ButtonHTMLAttributes } from "react";
import { cn } from "./utils";

type ButtonProps = ButtonHTMLAttributes<HTMLButtonElement> & {
  variant?: "default" | "ghost" | "outline";
  size?: "default" | "icon";
};

export function Button({ className, variant = "default", size = "default", ...props }: ButtonProps) {
  return (
    <button
      className={cn(
        "inline-flex items-center justify-center gap-2 rounded-md text-sm font-medium transition-[color,background-color,border-color,box-shadow,transform] duration-150 ease-out focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring/35 disabled:pointer-events-none disabled:opacity-50",
        variant === "default" && "bg-primary text-primary-foreground shadow-sm hover:bg-primary/90",
        variant === "ghost" && "text-muted-foreground hover:bg-muted hover:text-foreground",
        variant === "outline" && "border border-border bg-card text-foreground hover:border-input hover:bg-muted/60",
        size === "default" && "h-9 px-3.5",
        size === "icon" && "h-9 w-9",
        className
      )}
      {...props}
    />
  );
}
