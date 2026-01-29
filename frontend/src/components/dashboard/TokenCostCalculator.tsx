// frontend/src/components/dashboard/TokenCostCalculator.tsx
import { useTranslation } from "react-i18next";
import {
  Paper,
  Text,
  Group,
  Stack,
  Table,
  Badge,
  Tooltip,
  ThemeIcon,
  Skeleton,
} from "@mantine/core";
import { IconCurrencyDollar, IconInfoCircle } from "@tabler/icons-react";

// Claude model pricing per million tokens (as of 2024)
const MODEL_PRICING = {
  "claude-3-opus": { input: 15.0, output: 75.0 },
  "claude-3-sonnet": { input: 3.0, output: 15.0 },
  "claude-3-haiku": { input: 0.25, output: 1.25 },
  "claude-3.5-sonnet": { input: 3.0, output: 15.0 },
  "claude-3.5-haiku": { input: 0.8, output: 4.0 },
  "claude-sonnet-4": { input: 3.0, output: 15.0 },
  "claude-opus-4": { input: 15.0, output: 75.0 },
} as const;

interface TokenUsage {
  model: string;
  inputTokens: number;
  outputTokens: number;
  timestamp?: string;
}

interface TokenCostCalculatorProps {
  usage?: TokenUsage[];
  totalInputTokens?: number;
  totalOutputTokens?: number;
  isLoading?: boolean;
}

function formatCurrency(amount: number): string {
  if (amount < 0.01) {
    return `$${amount.toFixed(4)}`;
  }
  return `$${amount.toFixed(2)}`;
}

function formatTokenCount(count: number): string {
  if (count >= 1_000_000) {
    return `${(count / 1_000_000).toFixed(2)}M`;
  }
  if (count >= 1_000) {
    return `${(count / 1_000).toFixed(1)}K`;
  }
  return count.toString();
}

function calculateCost(
  model: string,
  inputTokens: number,
  outputTokens: number
): { inputCost: number; outputCost: number; totalCost: number } {
  const pricing = MODEL_PRICING[model as keyof typeof MODEL_PRICING] ||
    MODEL_PRICING["claude-3.5-sonnet"]; // Default fallback

  const inputCost = (inputTokens / 1_000_000) * pricing.input;
  const outputCost = (outputTokens / 1_000_000) * pricing.output;

  return {
    inputCost,
    outputCost,
    totalCost: inputCost + outputCost,
  };
}

export function TokenCostCalculator({
  usage = [],
  totalInputTokens = 0,
  totalOutputTokens = 0,
  isLoading,
}: TokenCostCalculatorProps) {
  const { t } = useTranslation();

  if (isLoading) {
    return (
      <Stack gap="md">
        <Skeleton height={20} width="50%" />
        <Skeleton height={100} />
      </Stack>
    );
  }

  // Calculate totals from usage array or use provided totals
  let inputTotal = totalInputTokens;
  let outputTotal = totalOutputTokens;

  if (usage.length > 0) {
    inputTotal = usage.reduce((sum, u) => sum + u.inputTokens, 0);
    outputTotal = usage.reduce((sum, u) => sum + u.outputTokens, 0);
  }

  // Default to claude-3.5-sonnet for cost estimation if no specific model
  const defaultModel = "claude-3.5-sonnet";
  const { inputCost, outputCost, totalCost } = calculateCost(
    defaultModel,
    inputTotal,
    outputTotal
  );

  const hasData = inputTotal > 0 || outputTotal > 0;

  return (
    <Stack gap="md">
      {/* Summary Cards */}
      <Group grow>
        <Paper p="md" radius="md" withBorder>
          <Group gap="xs" mb="xs">
            <ThemeIcon size="sm" variant="light" color="blue" radius="xl">
              <IconCurrencyDollar size={14} />
            </ThemeIcon>
            <Text size="xs" c="dimmed" tt="uppercase" fw={500}>
              {t("dashboard.tokenCost.estimatedCost")}
            </Text>
            <Tooltip label={t("dashboard.tokenCost.basedOnSonnet")}>
              <ThemeIcon size="xs" variant="subtle" color="gray">
                <IconInfoCircle size={12} />
              </ThemeIcon>
            </Tooltip>
          </Group>
          <Text size="xl" fw={700} c="green">
            {hasData ? formatCurrency(totalCost) : "-"}
          </Text>
        </Paper>
      </Group>

      {/* Token Breakdown */}
      <Table withTableBorder withColumnBorders>
        <Table.Thead>
          <Table.Tr>
            <Table.Th>{t("dashboard.tokenCost.type")}</Table.Th>
            <Table.Th style={{ textAlign: "right" }}>
              {t("dashboard.tokenCost.tokens")}
            </Table.Th>
            <Table.Th style={{ textAlign: "right" }}>
              {t("dashboard.tokenCost.cost")}
            </Table.Th>
          </Table.Tr>
        </Table.Thead>
        <Table.Tbody>
          <Table.Tr>
            <Table.Td>
              <Group gap="xs">
                <Badge size="sm" variant="light" color="blue">
                  Input
                </Badge>
              </Group>
            </Table.Td>
            <Table.Td style={{ textAlign: "right", fontFamily: "monospace" }}>
              {formatTokenCount(inputTotal)}
            </Table.Td>
            <Table.Td style={{ textAlign: "right", fontFamily: "monospace" }}>
              {formatCurrency(inputCost)}
            </Table.Td>
          </Table.Tr>
          <Table.Tr>
            <Table.Td>
              <Group gap="xs">
                <Badge size="sm" variant="light" color="violet">
                  Output
                </Badge>
              </Group>
            </Table.Td>
            <Table.Td style={{ textAlign: "right", fontFamily: "monospace" }}>
              {formatTokenCount(outputTotal)}
            </Table.Td>
            <Table.Td style={{ textAlign: "right", fontFamily: "monospace" }}>
              {formatCurrency(outputCost)}
            </Table.Td>
          </Table.Tr>
          <Table.Tr style={{ fontWeight: 600 }}>
            <Table.Td>
              <Text fw={600}>{t("dashboard.tokenCost.total")}</Text>
            </Table.Td>
            <Table.Td style={{ textAlign: "right", fontFamily: "monospace" }}>
              <Text fw={600}>{formatTokenCount(inputTotal + outputTotal)}</Text>
            </Table.Td>
            <Table.Td style={{ textAlign: "right", fontFamily: "monospace" }}>
              <Text fw={600} c="green">
                {formatCurrency(totalCost)}
              </Text>
            </Table.Td>
          </Table.Tr>
        </Table.Tbody>
      </Table>

      {/* Model Comparison */}
      {hasData && (
        <Stack gap="xs">
          <Text size="xs" c="dimmed" tt="uppercase" fw={500}>
            {t("dashboard.tokenCost.modelComparison")}
          </Text>
          <Table withTableBorder>
            <Table.Thead>
              <Table.Tr>
                <Table.Th>{t("dashboard.tokenCost.model")}</Table.Th>
                <Table.Th style={{ textAlign: "right" }}>
                  {t("dashboard.tokenCost.estimatedCost")}
                </Table.Th>
              </Table.Tr>
            </Table.Thead>
            <Table.Tbody>
              {Object.entries(MODEL_PRICING).map(([model]) => {
                const cost = calculateCost(model, inputTotal, outputTotal);
                return (
                  <Table.Tr key={model}>
                    <Table.Td>
                      <Text size="sm" ff="monospace">
                        {model}
                      </Text>
                    </Table.Td>
                    <Table.Td style={{ textAlign: "right" }}>
                      <Text size="sm" ff="monospace">
                        {formatCurrency(cost.totalCost)}
                      </Text>
                    </Table.Td>
                  </Table.Tr>
                );
              })}
            </Table.Tbody>
          </Table>
        </Stack>
      )}
    </Stack>
  );
}

export default TokenCostCalculator;
