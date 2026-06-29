import type { Meta, StoryObj } from "@storybook/react";
import { ScoreTrendChart } from "./ScoreTrendChart";
import {
  DEFAULT_FIXTURES,
  createInMemoryScoreHistoryAdapter,
} from "../lib/score-history/adapter";

// The chart pulls its data through a typed adapter so stories can drive each
// state by passing the in-memory adapter with the relevant fixture key. The
// permalink work (issue #364) will swap the real adapter at the dashboard
// call site without touching the component.
const adapter = createInMemoryScoreHistoryAdapter(DEFAULT_FIXTURES);

const meta: Meta<typeof ScoreTrendChart> = {
  title: "Components/ScoreTrendChart",
  component: ScoreTrendChart,
  tags: ["autodocs"],
  parameters: {
    layout: "padded",
    docs: {
      description: {
        component:
          "Historical Sanctity Score line chart for a tracked contract. Severity bands render as ReferenceArea overlays so the line stays the focal point. Range selector pins to 7d, 30d, or all and collapses to all when the dataset is shorter than the requested window.",
      },
    },
  },
  args: {
    adapter,
  },
};

export default meta;
type Story = StoryObj<typeof ScoreTrendChart>;

export const Healthy: Story = {
  name: "Healthy trend",
  args: {
    contractId: "demo-healthy",
  },
};

export const Regression: Story = {
  name: "Regression spike",
  args: {
    contractId: "demo-regression",
  },
};

export const SinglePoint: Story = {
  name: "Single point",
  args: {
    contractId: "demo-single",
  },
};

export const Empty: Story = {
  name: "Empty history",
  args: {
    contractId: "demo-empty",
  },
};
