import { ReactNode, useEffect, useId, useRef, useState } from "react";

export interface MenuItem {
  label: string;
  icon?: ReactNode;
  onSelect: () => void;
  /** Shown on the right, e.g. a check mark for the current choice. */
  trailing?: ReactNode;
  danger?: boolean;
  disabled?: boolean;
}

interface Props {
  /** Accessible name of the trigger button. */
  label: string;
  trigger: ReactNode;
  items: (MenuItem | "separator")[];
  heading?: string;
  align?: "start" | "end";
  triggerClassName?: string;
  disabled?: boolean;
}

/** A button that opens a keyboard-operable menu: arrows move, Enter selects, Escape closes. */
export function Menu({ label, trigger, items, heading, align = "start", triggerClassName = "chip", disabled }: Props) {
  const [open, setOpen] = useState(false);
  const root = useRef<HTMLDivElement>(null);
  const button = useRef<HTMLButtonElement>(null);
  const menuId = useId();

  const enabledItems = () => Array.from(root.current?.querySelectorAll<HTMLButtonElement>('[role="menuitem"]:not(:disabled)') ?? []);

  useEffect(() => {
    if (!open) return;
    enabledItems()[0]?.focus();
    const onDown = (e: MouseEvent) => {
      if (!root.current?.contains(e.target as Node)) setOpen(false);
    };
    document.addEventListener("mousedown", onDown);
    return () => document.removeEventListener("mousedown", onDown);
  }, [open]);

  const close = (refocus = true) => {
    setOpen(false);
    if (refocus) button.current?.focus();
  };

  const onKeyDown = (e: React.KeyboardEvent) => {
    const list = enabledItems();
    const i = list.indexOf(document.activeElement as HTMLButtonElement);
    if (e.key === "ArrowDown") list[(i + 1) % list.length]?.focus();
    else if (e.key === "ArrowUp") list[(i - 1 + list.length) % list.length]?.focus();
    else if (e.key === "Home") list[0]?.focus();
    else if (e.key === "End") list[list.length - 1]?.focus();
    else if (e.key === "Escape") close();
    else if (e.key === "Tab") close(false);
    else return;
    e.preventDefault();
  };

  return (
    <div className="menu-root" ref={root} onKeyDown={open ? onKeyDown : undefined}>
      <button
        ref={button}
        className={triggerClassName}
        aria-label={label}
        aria-haspopup="menu"
        aria-expanded={open}
        aria-controls={open ? menuId : undefined}
        disabled={disabled}
        onClick={() => setOpen((o) => !o)}
        onKeyDown={(e) => {
          if (e.key === "ArrowDown" && !open) {
            e.preventDefault();
            setOpen(true);
          }
        }}
      >
        {trigger}
      </button>
      {open && (
        <div className={`menu menu-${align}`} role="menu" id={menuId} aria-label={label}>
          {heading && <div className="menu-heading" aria-hidden>{heading}</div>}
          {items.map((item, i) =>
            item === "separator" ? (
              <div key={`sep-${i}`} className="menu-separator" role="separator" />
            ) : (
              <button
                key={item.label}
                role="menuitem"
                className={`menu-item${item.danger ? " danger" : ""}`}
                disabled={item.disabled}
                onClick={() => {
                  close();
                  item.onSelect();
                }}
              >
                <span className="menu-icon" aria-hidden>{item.icon}</span>
                <span className="menu-label">{item.label}</span>
                {item.trailing && <span className="menu-trailing">{item.trailing}</span>}
              </button>
            ),
          )}
        </div>
      )}
    </div>
  );
}
