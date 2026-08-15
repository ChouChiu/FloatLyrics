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
  RiMusic2Line,
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
  | "info"
  | "music";

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
  music: RiMusic2Line,
};

export function Icon({ name }: { name: IconName }) {
  const RemixIcon = icons[name];
  return <RemixIcon aria-hidden="true" className="icon" />;
}
