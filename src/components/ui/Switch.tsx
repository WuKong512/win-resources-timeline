type SwitchProps = {
  checked: boolean;
  onCheckedChange: (checked: boolean) => void;
  ariaLabel: string;
  disabled?: boolean;
};

export function Switch({ checked, onCheckedChange, ariaLabel, disabled = false }: SwitchProps) {
  return (
    <button
      type="button"
      role="switch"
      aria-label={ariaLabel}
      aria-checked={checked}
      disabled={disabled}
      onClick={() => onCheckedChange(!checked)}
      className={`relative h-[22px] w-10 shrink-0 overflow-hidden rounded-full border p-0 align-middle transition-[background-color,border-color,transform] duration-150 ease-out focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring/30 disabled:cursor-not-allowed disabled:opacity-50 ${checked ? "border-primary bg-primary" : "border-border bg-muted"}`}
    >
      <span className={`absolute left-0 top-0.5 h-4 w-4 rounded-full bg-primary-foreground shadow-sm transition-transform duration-150 ease-out ${checked ? "translate-x-[20px]" : "translate-x-0.5"}`} />
    </button>
  );
}
