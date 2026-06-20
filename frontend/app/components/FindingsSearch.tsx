"use client";

import { useEffect, useState } from "react";

interface FindingsSearchProps {
  /** The committed search term (source of truth, e.g. from the URL). */
  value: string;
  /** Called with the debounced value. */
  onChange: (value: string) => void;
  /** Debounce delay in ms. */
  delay?: number;
  placeholder?: string;
}

/**
 * Search box that keeps a responsive local value while debouncing the committed
 * `onChange` so we don't rewrite the URL on every keystroke. Re-syncs to `value`
 * when it changes externally (e.g. "Clear filters").
 */
export function FindingsSearch({
  value,
  onChange,
  delay = 250,
  placeholder = "Search findings…",
}: FindingsSearchProps) {
  const [local, setLocal] = useState(value);

  // Re-sync when the committed value changes from the outside.
  useEffect(() => {
    setLocal(value);
  }, [value]);

  // Debounce upward commits.
  useEffect(() => {
    if (local === value) return;
    const id = setTimeout(() => onChange(local), delay);
    return () => clearTimeout(id);
  }, [local, value, delay, onChange]);

  return (
    <div className="relative w-full sm:w-72">
      <input
        type="search"
        role="searchbox"
        value={local}
        onChange={(e) => setLocal(e.target.value)}
        placeholder={placeholder}
        aria-label="Search findings"
        className="w-full rounded-lg border border-zinc-300 dark:border-zinc-600 theme-high-contrast:border-white bg-white dark:bg-zinc-950 theme-high-contrast:bg-black px-3 py-1.5 text-sm outline-none focus:ring-2 focus:ring-zinc-400 dark:focus:ring-zinc-600"
      />
    </div>
  );
}
