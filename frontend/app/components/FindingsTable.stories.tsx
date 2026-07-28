import type { Meta, StoryObj } from "@storybook/react";
import { useState } from "react";
import { FindingsTable } from "./FindingsTable";
import type { Finding, Severity } from "../types";
import type { SortDir, SortKey } from "../lib/findings-query";

const severities: Severity[] = ["critical", "high", "medium", "low"];
const categories = ["Auth Gap", "Panic/Unwrap", "Arithmetic", "Ledger Size", "Unsafe Pattern"];

function makeFindings(count: number): Finding[] {
  return Array.from({ length: count }, (_, i) => {
    const severity = severities[i % severities.length];
    const category = categories[i % categories.length];
    return {
      id: `f-${i}`,
      severity,
      category,
      title: `${category} detected in handler #${i}`,
      location: `src/lib.rs:${10 + i}`,
      snippet: i % 3 === 0 ? `let value = input.parse::<u64>().unwrap(); // row ${i}` : undefined,
      line: 1,
      suggestion: i % 2 === 0 ? "Handle the error path explicitly." : undefined,
      raw: null,
    };
  });
}

/** Interactive wrapper so the sort headers actually re-sort within the story. */
function TableHarness({ rows }: { rows: Finding[] }) {
  const [sort, setSort] = useState<SortKey | null>("severity");
  const [dir, setDir] = useState<SortDir>("desc");
  const [selected, setSelected] = useState<Finding | null>(null);
  return (
    <>
      <FindingsTable
        rows={rows}
        sort={sort}
        dir={dir}
        onSortChange={(s, d) => {
          setSort(s);
          setDir(d);
        }}
        onRowClick={setSelected}
      />
      {selected && (
        <p style={{ marginTop: 8, fontSize: 12, color: "#71717a" }}>
          Selected: {selected.id} — {selected.title}
        </p>
      )}
    </>
  );
}

const meta: Meta<typeof FindingsTable> = {
  title: "Components/FindingsTable",
  component: FindingsTable,
  tags: ["autodocs"],
  parameters: {
    layout: "padded",
    docs: {
      description: {
        component:
          "Virtualized, sortable findings table (TanStack Table + Virtual). Columns: severity, code, file:line, message. Clicking a row fires onRowClick so the parent can open a detail drawer. Sorting is controlled by the caller so it can be persisted to the URL.",
      },
    },
  },
};

export default meta;
type Story = StoryObj<typeof FindingsTable>;

export const Default: Story = {
  render: () => <TableHarness rows={makeFindings(25)} />,
};

/** Demonstrates virtualization staying smooth at 500+ rows. */
export const ManyRows: Story = {
  render: () => <TableHarness rows={makeFindings(600)} />,
};
