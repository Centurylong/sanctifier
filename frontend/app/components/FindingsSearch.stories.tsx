import type { Meta, StoryObj } from "@storybook/react";
import { useState } from "react";
import { FindingsSearch } from "./FindingsSearch";

function Harness() {
  const [value, setValue] = useState("");
  return (
    <div className="space-y-2">
      <FindingsSearch value={value} onChange={setValue} />
      <p className="text-sm text-zinc-500">
        Debounced value: <code>{value || "(empty)"}</code>
      </p>
    </div>
  );
}

const meta: Meta<typeof FindingsSearch> = {
  title: "Components/FindingsSearch",
  component: FindingsSearch,
  tags: ["autodocs"],
  parameters: {
    layout: "padded",
    docs: {
      description: {
        component:
          "Search input that keeps a responsive local value while debouncing the committed onChange (default 250ms), so the URL isn't rewritten on every keystroke.",
      },
    },
  },
};

export default meta;
type Story = StoryObj<typeof FindingsSearch>;

export const Default: Story = {
  render: () => <Harness />,
};
