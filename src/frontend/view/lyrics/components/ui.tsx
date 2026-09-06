// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

import {
  type RemixiconComponentType,
  RiAddLine,
  RiArrowDownLine,
  RiArrowUpLine,
  RiCloseLine,
  RiDatabase2Line,
  RiDeleteBin6Line,
  RiInformationLine,
  RiLayoutLine,
  RiSearchLine,
  RiSettings3Line,
  RiSubtractLine,
} from "@remixicon/react";
import {
  type ButtonHTMLAttributes,
  type InputHTMLAttributes,
  type ReactNode,
  type SelectHTMLAttributes,
  useEffect,
  useState,
} from "react";

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

export function Select({ className, children, ...props }: SelectHTMLAttributes<HTMLSelectElement>) {
  return (
    <select className={classes("select", className)} {...props}>
      {children}
    </select>
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
  onValueChange,
}: {
  value: number;
  min: number;
  max: number;
  step?: number;
  unit?: string;
  label: string;
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
    <div className="slider-row">
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
  children,
}: {
  title: string;
  description: string;
  children: ReactNode;
}) {
  return (
    <div className="setting-row">
      <div className="setting-copy">
        <div className="setting-title">{title}</div>
        <div className="setting-description">{description}</div>
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
  | "sources"
  | "up"
  | "down"
  | "remove"
  | "x"
  | "info";

const icons: Record<IconName, RemixiconComponentType> = {
  minus: RiSubtractLine,
  plus: RiAddLine,
  search: RiSearchLine,
  settings: RiSettings3Line,
  display: RiLayoutLine,
  sources: RiDatabase2Line,
  up: RiArrowUpLine,
  down: RiArrowDownLine,
  remove: RiDeleteBin6Line,
  x: RiCloseLine,
  info: RiInformationLine,
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
