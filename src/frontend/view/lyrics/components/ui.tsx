// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

import {
  type RemixiconComponentType,
  RiAddLine,
  RiArrowDownLine,
  RiArrowDownSLine,
  RiArrowGoBackLine,
  RiArrowUpLine,
  RiCheckLine,
  RiCloseLine,
  RiDatabase2Line,
  RiDeleteBin6Line,
  RiInformationLine,
  RiLayoutLine,
  RiLinkM,
  RiSearchLine,
  RiSettings3Line,
  RiSubtractLine,
} from "@remixicon/react";
import {
  type ButtonHTMLAttributes,
  type InputHTMLAttributes,
  type KeyboardEvent,
  type ReactNode,
  useEffect,
  useId,
  useLayoutEffect,
  useRef,
  useState,
} from "react";
import { createPortal } from "react-dom";

function classes(...values: Array<string | false | null | undefined>): string {
  return values.filter(Boolean).join(" ");
}

export function Button({
  className,
  variant = "default",
  size = "default",
  ...props
}: ButtonHTMLAttributes<HTMLButtonElement> & {
  variant?: "default" | "ghost" | "outline" | "destructive";
  size?: "default" | "icon" | "sm";
}) {
  return (
    <button
      type="button"
      className={classes("button", `button-${variant}`, `button-${size}`, className)}
      {...props}
    />
  );
}

export function Card({ className, children }: { className?: string; children: ReactNode }) {
  return <section className={classes("card", className)}>{children}</section>;
}

export function Input({ className, ...props }: InputHTMLAttributes<HTMLInputElement>) {
  return <input className={classes("input", className)} {...props} />;
}

export interface SelectOption<T extends string> {
  value: T;
  label: string;
}

interface SelectPosition {
  left: number;
  width: number;
  top?: number;
  bottom?: number;
  maxHeight: number;
}

const SELECT_GAP = 6;
const SELECT_MARGIN = 8;

function selectPosition(trigger: HTMLElement): SelectPosition {
  const rect = trigger.getBoundingClientRect();
  const below = window.innerHeight - rect.bottom - SELECT_GAP - SELECT_MARGIN;
  const above = rect.top - SELECT_GAP - SELECT_MARGIN;
  const base = { left: rect.left, width: rect.width };
  // Open downwards unless the space above is clearly the better fit.
  return below >= 160 || below >= above
    ? { ...base, top: rect.bottom + SELECT_GAP, maxHeight: below }
    : { ...base, bottom: window.innerHeight - rect.top + SELECT_GAP, maxHeight: above };
}

/**
 * A dropdown drawn by the page itself.
 *
 * WebKitGTK renders a native `<select>` popup as a GTK popover styled by the
 * desktop theme, which does not match the rest of the control center.
 */
export function Select<T extends string>({
  value,
  options,
  onValueChange,
  disabled,
  label,
  className,
}: {
  value: T;
  options: ReadonlyArray<SelectOption<T>>;
  onValueChange(value: T): void;
  disabled?: boolean;
  label?: string;
  className?: string;
}) {
  const listId = useId();
  const triggerRef = useRef<HTMLButtonElement>(null);
  const listRef = useRef<HTMLDivElement>(null);
  const [open, setOpen] = useState(false);
  const [position, setPosition] = useState<SelectPosition | null>(null);
  const selectedIndex = options.findIndex((option) => option.value === value);
  const [highlighted, setHighlighted] = useState(selectedIndex);
  const selected = options[selectedIndex];

  useLayoutEffect(() => {
    if (!open || !triggerRef.current) return;
    setPosition(selectPosition(triggerRef.current));
  }, [open]);

  useEffect(() => {
    if (!open) return;
    const close = () => setOpen(false);
    const onPointerDown = (event: PointerEvent) => {
      const target = event.target as Node;
      if (triggerRef.current?.contains(target) || listRef.current?.contains(target)) return;
      close();
    };
    const onScroll = (event: Event) => {
      if (listRef.current?.contains(event.target as Node)) return;
      close();
    };
    document.addEventListener("pointerdown", onPointerDown, true);
    window.addEventListener("scroll", onScroll, true);
    window.addEventListener("resize", close);
    window.addEventListener("blur", close);
    return () => {
      document.removeEventListener("pointerdown", onPointerDown, true);
      window.removeEventListener("scroll", onScroll, true);
      window.removeEventListener("resize", close);
      window.removeEventListener("blur", close);
    };
  }, [open]);

  useEffect(() => {
    if (open)
      document.getElementById(`${listId}-${highlighted}`)?.scrollIntoView({ block: "nearest" });
  }, [open, highlighted, listId]);

  useEffect(() => {
    if (disabled) setOpen(false);
  }, [disabled]);

  const openList = () => {
    setHighlighted(Math.max(selectedIndex, 0));
    setOpen(true);
  };

  const choose = (index: number) => {
    const option = options[index];
    setOpen(false);
    triggerRef.current?.focus();
    if (option && option.value !== value) onValueChange(option.value);
  };

  const onKeyDown = (event: KeyboardEvent<HTMLButtonElement>) => {
    if (disabled || options.length === 0) return;
    const last = options.length - 1;
    const move = (index: number) => {
      event.preventDefault();
      if (open) setHighlighted(index);
      else if (options[index]) onValueChange(options[index].value);
    };
    switch (event.key) {
      case "ArrowDown":
        if (event.altKey && !open) {
          event.preventDefault();
          return openList();
        }
        return move(Math.min((open ? highlighted : selectedIndex) + 1, last));
      case "ArrowUp":
        return move(Math.max((open ? highlighted : selectedIndex) - 1, 0));
      case "Home":
        return move(0);
      case "End":
        return move(last);
      case "Enter":
      case " ":
        event.preventDefault();
        return open ? choose(highlighted) : openList();
      case "Escape":
      case "Tab":
        if (open) {
          if (event.key === "Escape") event.preventDefault();
          setOpen(false);
        }
        return;
    }
  };

  return (
    <>
      <button
        ref={triggerRef}
        type="button"
        role="combobox"
        aria-label={label}
        aria-haspopup="listbox"
        aria-expanded={open}
        aria-controls={open ? listId : undefined}
        aria-activedescendant={open ? `${listId}-${highlighted}` : undefined}
        disabled={disabled}
        className={classes("select", className)}
        data-state={open ? "open" : "closed"}
        onClick={() => (open ? setOpen(false) : openList())}
        onKeyDown={onKeyDown}
      >
        <span className="select-value">{selected?.label ?? ""}</span>
        <RiArrowDownSLine aria-hidden="true" className="icon select-chevron" />
      </button>
      {open &&
        position &&
        createPortal(
          <div
            ref={listRef}
            id={listId}
            role="listbox"
            aria-label={label}
            className="select-content"
            style={position}
          >
            {options.map((option, index) => (
              // biome-ignore lint/a11y/useKeyWithClickEvents: the combobox trigger keeps focus and handles the keyboard
              <div
                key={option.value}
                id={`${listId}-${index}`}
                role="option"
                tabIndex={-1}
                aria-selected={option.value === value}
                className="select-item"
                data-highlighted={index === highlighted}
                onPointerMove={() => setHighlighted(index)}
                onMouseDown={(event) => event.preventDefault()}
                onClick={() => choose(index)}
              >
                <span>{option.label}</span>
                {option.value === value && (
                  <RiCheckLine aria-hidden="true" className="icon select-check" />
                )}
              </div>
            ))}
          </div>,
          document.body,
        )}
    </>
  );
}

export function Switch({
  checked,
  onCheckedChange,
  disabled,
  label,
}: {
  checked: boolean;
  onCheckedChange(value: boolean): void;
  disabled?: boolean;
  label: string;
}) {
  return (
    <button
      type="button"
      role="switch"
      aria-label={label}
      aria-checked={checked}
      disabled={disabled}
      className="switch"
      data-state={checked ? "checked" : "unchecked"}
      onClick={() => onCheckedChange(!checked)}
    >
      <span className="switch-thumb" />
    </button>
  );
}

export function Slider({
  value,
  min,
  max,
  step = 1,
  unit,
  label,
  className,
  onValueChange,
}: {
  value: number;
  min: number;
  max: number;
  step?: number;
  unit?: string;
  label: string;
  className?: string;
  onValueChange(value: number): void;
}) {
  const [numberValue, setNumberValue] = useState(String(value));
  useEffect(() => setNumberValue(String(value)), [value]);
  const isValidNumber = (candidate: number) =>
    Number.isFinite(candidate) &&
    candidate >= min &&
    candidate <= max &&
    (!Number.isInteger(step) || Number.isInteger(candidate));

  const updateNumber = (raw: string) => {
    setNumberValue(raw);
    if (raw.trim() === "") return;
    const next = Number(raw);
    if (isValidNumber(next)) onValueChange(next);
  };

  return (
    <div className={classes("slider-row", className)}>
      <input
        aria-label={label}
        className="slider"
        type="range"
        value={value}
        min={min}
        max={max}
        step={step}
        onChange={(event) => onValueChange(Number(event.currentTarget.value))}
      />
      <div className="slider-number">
        <input
          aria-label={label}
          className="slider-number-input"
          type="number"
          inputMode="decimal"
          value={numberValue}
          min={min}
          max={max}
          step={step}
          onChange={(event) => updateNumber(event.currentTarget.value)}
          onBlur={() => {
            const parsed = Number(numberValue);
            setNumberValue(
              numberValue.trim() !== "" && isValidNumber(parsed) ? String(parsed) : String(value),
            );
          }}
        />
        {unit && <span>{unit.trim()}</span>}
      </div>
    </div>
  );
}

export function SettingRow({
  title,
  description,
  disabled,
  children,
}: {
  title: string;
  description?: string;
  disabled?: boolean;
  children: ReactNode;
}) {
  return (
    <div className="setting-row" data-disabled={disabled || undefined}>
      <div className="setting-copy">
        <div className="setting-title">{title}</div>
        {description && <div className="setting-description">{description}</div>}
      </div>
      <div className="setting-control">{children}</div>
    </div>
  );
}

type IconName =
  | "minus"
  | "plus"
  | "search"
  | "settings"
  | "display"
  | "integration"
  | "sources"
  | "up"
  | "down"
  | "remove"
  | "x"
  | "info"
  | "reset";

const icons: Record<IconName, RemixiconComponentType> = {
  minus: RiSubtractLine,
  plus: RiAddLine,
  search: RiSearchLine,
  settings: RiSettings3Line,
  display: RiLayoutLine,
  integration: RiLinkM,
  sources: RiDatabase2Line,
  up: RiArrowUpLine,
  down: RiArrowDownLine,
  remove: RiDeleteBin6Line,
  x: RiCloseLine,
  info: RiInformationLine,
  reset: RiArrowGoBackLine,
};

export function Icon({ name }: { name: IconName }) {
  const RemixIcon = icons[name];
  return <RemixIcon aria-hidden="true" className="icon" />;
}

export function AppIcon({ className }: { className?: string }) {
  return (
    <svg
      className={className}
      viewBox="0 0 512 512"
      xmlns="http://www.w3.org/2000/svg"
      role="img"
      aria-label="FloatLyrics"
    >
      <defs>
        <linearGradient
          id="app-icon-accent"
          gradientUnits="userSpaceOnUse"
          x1="140"
          y1="300"
          x2="380"
          y2="180"
        >
          <stop offset="0" stopColor="#2ee0ff" />
          <stop offset="0.5" stopColor="#7a6bff" />
          <stop offset="1" stopColor="#e34bff" />
        </linearGradient>
        <linearGradient
          id="app-icon-backdrop"
          gradientUnits="userSpaceOnUse"
          x1="40"
          y1="40"
          x2="400"
          y2="472"
        >
          <stop offset="0" stopColor="#1b2057" />
          <stop offset="1" stopColor="#0b0d24" />
        </linearGradient>
      </defs>
      <rect x="40" y="40" width="432" height="432" rx="108" fill="url(#app-icon-backdrop)" />
      <g fill="none" strokeLinecap="round">
        <path d="M150 170h150" stroke="#5b638f" strokeWidth="24" />
        <path d="M150 256h212" stroke="url(#app-icon-accent)" strokeWidth="42" />
        <path d="M150 342h124" stroke="#5b638f" strokeWidth="24" />
      </g>
    </svg>
  );
}
