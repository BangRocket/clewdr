import React from "react";
import {
  CartesianGrid,
  Line,
  LineChart,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from "recharts";

import type { TimeBucket } from "../../../types/usage.types";

interface Props {
  data: TimeBucket[];
  height?: number;
  /** Hide axes/grid for sparkline mode */
  minimal?: boolean;
}

const fmtTs = (ts: number) =>
  new Date(ts * 1000).toLocaleDateString(undefined, {
    month: "short",
    day: "numeric",
  });

const fmtCost = (v: number) => `$${v.toFixed(2)}`;
const fmtCostPrecise = (v: number) => `$${v.toFixed(4)}`;

const CostLineChart: React.FC<Props> = ({ data, height = 240, minimal = false }) => {
  if (data.length === 0) {
    return (
      <div
        style={{ height }}
        className="flex items-center justify-center text-sm text-gray-500"
      >
        No data
      </div>
    );
  }
  return (
    <ResponsiveContainer width="100%" height={height}>
      <LineChart data={data} margin={{ top: 8, right: 8, bottom: 8, left: 0 }}>
        {!minimal && (
          <>
            <CartesianGrid strokeDasharray="3 3" stroke="#374151" />
            <XAxis
              dataKey="ts"
              tickFormatter={fmtTs}
              stroke="#9ca3af"
              tick={{ fontSize: 11 }}
            />
            <YAxis
              tickFormatter={fmtCost}
              stroke="#9ca3af"
              tick={{ fontSize: 11 }}
              width={50}
            />
            <Tooltip
              formatter={(v) => [fmtCostPrecise(Number(v)), "cost"]}
              labelFormatter={(ts) => fmtTs(Number(ts))}
              contentStyle={{
                backgroundColor: "#1f2937",
                border: "1px solid #374151",
                borderRadius: 6,
                fontSize: 12,
              }}
            />
          </>
        )}
        <Line
          type="monotone"
          dataKey="cost_usd"
          stroke="#60a5fa"
          strokeWidth={2}
          dot={!minimal}
          isAnimationActive={!minimal}
        />
      </LineChart>
    </ResponsiveContainer>
  );
};

export default CostLineChart;
