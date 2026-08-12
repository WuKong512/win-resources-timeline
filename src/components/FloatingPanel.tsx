import { useLayoutEffect, useRef, useState, type ReactNode, type RefObject } from "react";
import { createPortal } from "react-dom";

type FloatingPanelProps = {
  open: boolean;
  anchorRef: RefObject<HTMLElement>;
  onClose: () => void;
  width: number;
  align?: "start" | "end";
  children: ReactNode;
  className?: string;
  ariaLabel: string;
};

export function FloatingPanel({
  open,
  anchorRef,
  onClose,
  width,
  align = "start",
  children,
  className = "",
  ariaLabel
}: FloatingPanelProps) {
  const panelRef = useRef<HTMLDivElement | null>(null);
  const onCloseRef = useRef(onClose);
  const [position, setPosition] = useState({ left: 12, top: 12 });
  onCloseRef.current = onClose;

  useLayoutEffect(() => {
    if (!open) return;

    const updatePosition = () => {
      const anchor = anchorRef.current;
      if (!anchor) return;
      const rect = anchor.getBoundingClientRect();
      const panelHeight = panelRef.current?.offsetHeight ?? 360;
      const margin = 12;
      const gap = 8;
      const preferredLeft = align === "end" ? rect.right - width : rect.left;
      const maxLeft = Math.max(margin, window.innerWidth - width - margin);
      const left = Math.min(Math.max(margin, preferredLeft), maxLeft);
      const below = rect.bottom + gap;
      const above = rect.top - gap - panelHeight;
      const top = below + panelHeight <= window.innerHeight - margin || above < margin
        ? Math.min(below, Math.max(margin, window.innerHeight - panelHeight - margin))
        : above;
      setPosition({ left, top });
    };

    const closeOnOutsideClick = (event: MouseEvent) => {
      const target = event.target as Node;
      if (!anchorRef.current?.contains(target) && !panelRef.current?.contains(target)) onCloseRef.current();
    };
    const closeOnEscape = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        onCloseRef.current();
        window.requestAnimationFrame(() => anchorRef.current?.querySelector("button")?.focus());
      }
    };

    updatePosition();
    const frame = window.requestAnimationFrame(updatePosition);
    window.addEventListener("resize", updatePosition);
    window.addEventListener("scroll", updatePosition, true);
    document.addEventListener("mousedown", closeOnOutsideClick);
    document.addEventListener("keydown", closeOnEscape);
    return () => {
      window.cancelAnimationFrame(frame);
      window.removeEventListener("resize", updatePosition);
      window.removeEventListener("scroll", updatePosition, true);
      document.removeEventListener("mousedown", closeOnOutsideClick);
      document.removeEventListener("keydown", closeOnEscape);
    };
  }, [align, anchorRef, open, width]);

  if (!open) return null;
  return createPortal(
    <div
      ref={panelRef}
      role="dialog"
      aria-modal="false"
      aria-label={ariaLabel}
      className={`floating-panel-enter fixed z-[200] rounded-lg border border-border bg-card shadow-[0_18px_50px_hsl(var(--shadow-color)/0.28)] ${align === "end" ? "origin-top-right" : "origin-top-left"} ${className}`}
      style={{ left: position.left, top: position.top, width }}
    >
      {children}
    </div>,
    document.body
  );
}
