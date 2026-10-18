// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

// Ordered preference lists (font fallbacks, lyrics sources) are edited through
// these helpers. Each returns the input unchanged when the edit does not apply,
// so a caller can skip saving by comparing identity.

/** Appends `item` unless it is blank or already present. */
export function appendUnique<T>(items: T[], item: T): T[] {
  if ((typeof item === "string" && item.trim() === "") || items.includes(item)) return items;
  return [...items, item];
}

/** Swaps the item at `index` with its neighbour in direction `delta`. */
export function moveItem<T>(items: T[], index: number, delta: -1 | 1): T[] {
  const target = index + delta;
  if (index < 0 || index >= items.length || target < 0 || target >= items.length) return items;
  const next = [...items];
  [next[index], next[target]] = [next[target] as T, next[index] as T];
  return next;
}

/** Removes the item at `index`, always keeping at least one. */
export function removeItem<T>(items: T[], index: number): T[] {
  if (items.length <= 1 || index < 0 || index >= items.length) return items;
  return items.filter((_, current) => current !== index);
}
