import type { Meta, StoryObj } from "@storybook/react";
import { useState } from "react";
import { SeverityFacets } from "./SeverityFacets";
import type { Severity } from "../types";

function Harness({ initial }: { initial: Severity[] }) {
  const [selected, setSelected] = useState<Severity[]>(initial);
  return (
    <SeverityFacets
      selected={selected}
      counts={{ critical: 3, high: 7, medium: 12, low: 0 }}
      onToggle={(s) =>
        setSelected((prev) =>
          prev.includes(s) ? prev.filter((x) => x !== s) : [...prev, s]
        )
      }
      onClear={() => setSelected([])}
    />
  );
}

const meta: Meta<typeof SeverityFacets> = {
  title: "Components/SeverityFacets",
  component: SeverityFacets,
  tags: ["autodocs"],
  parameters: {
    layout: "padded",
    docs: {
      description: {
        component:
          "Multi-select severity facet chips with live counts. 'All' clears the selection; severities with a zero count are disabled.",
      },
    },
  },
};

export default meta;
type Story = StoryObj<typeof SeverityFacets>;

export const AllSelected: Story = {
  render: () => <Harness initial={[]} />,
};

export const SomeSelected: Story = {
  render: () => <Harness initial={["critical", "high"]} />,
};
