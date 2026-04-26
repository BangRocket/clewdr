import React from "react";
import {
  Bar,
  BarChart,
  CartesianGrid,
  Legend,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from "recharts";

import type { TimeBucket } from "../../../types/usage.types";

interface Props {
  data: TimeBucket[];
  height?: number;
}

const fmtTs = (ts: number) =>
  new Date(ts * 1000).toLocaleDateString(undefined, {
    month: "short",
    day: "numeric",
  });

const fmtTokens = (v: number) =>
  v >= 1_000_000
    ? `${(v / 1_000_000).toFixed(1)}M`
    : v >= 1_000
    ? `${(v / 1_000).toFixed(1)}k`
    : String(v);

const TokenBarChart: React.FC<Props> = ({ data, height = 240 }) => {
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
      <BarChart data={data} margin={{ top: 8, right: 8, bottom: 8, left: 0 }}>
        <CartesianGrid strokeDasharray="3 3" stroke="#374151" />
        <XAxis
          dataKey="ts"
          tickFormatter={fmtTs}
          stroke="#9ca3af"
          tick={{ fontSize: 11 }}
        />
        <YAxis
          tickFormatter={fmtTokens}
          stroke="#9ca3af"
          tick={{ fontSize: 11 }}
          width={50}
        />
        <Tooltip
          formatter={(v) => fmtTokens(Number(v))}
          labelFormatter={(ts) => fmtTs(Number(ts))}
          contentStyle={{
            backgroundColor: "#1f2937",
            border: "1px solid #374151",
            borderRadius: 6,
            fontSize: 12,
          }}
        />
        <Legend wrapperStyle={{ fontSize: 12 }} />
        <Bar dataKey="input_tokens" stackId="tokens" fill="#34d399" name="input" />
        <Bar dataKey="output_tokens" stackId="tokens" fill="#f59e0b" name="output" />
      </BarChart>
    </ResponsiveContainer>
  );
};

export default TokenBarChart;
