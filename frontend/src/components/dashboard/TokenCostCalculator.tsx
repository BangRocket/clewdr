// frontend/src/components/dashboard/TokenCostCalculator.tsx
import { useTranslation } from "react-i18next";
import {
  Text,
  Group,
  Stack,
  Table,
  Badge,
  Tooltip,
  ThemeIcon,
  Skeleton,
  Box,
} from "@mantine/core";
import { IconCurrencyDollar, IconInfoCircle } from "@tabler/icons-react";

// Claude 4.5 model family pricing per million tokens (2025)
const MODEL_PRICING = {
  "claude-opus-4.5": { input: 15.0, output: 75.0, color: "violet" },
  "claude-sonnet-4.5": { input: 3.0, output: 15.0, color: "cyan" },
  "claude-haiku-4.5": { input: 0.8, output: 4.0, color: "green" },
  // Legacy models for reference
  "claude-3-opus": { input: 15.0, output: 75.0, color: "grape" },
  "claude-3.5-sonnet": { input: 3.0, output: 15.0, color: "blue" },
  "claude-3.5-haiku": { input: 0.8, output: 4.0, color: "teal" },
} as const;

interface TokenCostCalculatorProps {
  totalInputTokens?: number;
  totalOutputTokens?: number;
  sonnetInputTokens?: number;
  sonnetOutputTokens?: number;
  opusInputTokens?: number;
  opusOutputTokens?: number;
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
    MODEL_PRICING["claude-sonnet-4.5"]; // Default fallback

  const inputCost = (inputTokens / 1_000_000) * pricing.input;
  const outputCost = (outputTokens / 1_000_000) * pricing.output;

  return {
    inputCost,
    outputCost,
    totalCost: inputCost + outputCost,
  };
}

export function TokenCostCalculator({
  totalInputTokens = 0,
  totalOutputTokens = 0,
  sonnetInputTokens = 0,
  sonnetOutputTokens = 0,
  opusInputTokens = 0,
  opusOutputTokens = 0,
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

  // Use model-specific tokens if available, otherwise use totals
  const inputTotal = totalInputTokens;
  const outputTotal = totalOutputTokens;

  // Calculate costs for the default model (Sonnet 4.5)
  const defaultModel = "claude-sonnet-4.5";
  const { inputCost, outputCost, totalCost } = calculateCost(
    defaultModel,
    inputTotal,
    outputTotal
  );

  // Calculate model-specific costs if we have the data
  const sonnetCost = calculateCost("claude-sonnet-4.5", sonnetInputTokens, sonnetOutputTokens);
  const opusCost = calculateCost("claude-opus-4.5", opusInputTokens, opusOutputTokens);

  const hasData = inputTotal > 0 || outputTotal > 0;
  const hasModelSpecific = sonnetInputTokens > 0 || opusInputTokens > 0;

  return (
    <Stack gap="md">
      {/* Summary Card */}
      <Box className="stat-card-green" p="md" style={{ borderRadius: 12 }}>
        <Group gap="xs" mb="xs">
          <ThemeIcon size="sm" variant="light" color="green" radius="xl">
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
        <Text size="xl" fw={700} c="white">
          {hasData ? formatCurrency(totalCost) : "-"}
        </Text>
      </Box>

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
                <Badge size="sm" variant="light" color="cyan">
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

      {/* Model-Specific Breakdown (if available) */}
      {hasModelSpecific && (
        <Stack gap="xs">
          <Text size="xs" c="dimmed" tt="uppercase" fw={500}>
            By Model
          </Text>
          <Table withTableBorder>
            <Table.Thead>
              <Table.Tr>
                <Table.Th>{t("dashboard.tokenCost.model")}</Table.Th>
                <Table.Th style={{ textAlign: "right" }}>Input</Table.Th>
                <Table.Th style={{ textAlign: "right" }}>Output</Table.Th>
                <Table.Th style={{ textAlign: "right" }}>
                  {t("dashboard.tokenCost.cost")}
                </Table.Th>
              </Table.Tr>
            </Table.Thead>
            <Table.Tbody>
              {sonnetInputTokens > 0 || sonnetOutputTokens > 0 ? (
                <Table.Tr>
                  <Table.Td>
                    <Badge size="sm" variant="light" color="cyan">
                      Sonnet 4.5
                    </Badge>
                  </Table.Td>
                  <Table.Td style={{ textAlign: "right", fontFamily: "monospace" }}>
                    {formatTokenCount(sonnetInputTokens)}
                  </Table.Td>
                  <Table.Td style={{ textAlign: "right", fontFamily: "monospace" }}>
                    {formatTokenCount(sonnetOutputTokens)}
                  </Table.Td>
                  <Table.Td style={{ textAlign: "right", fontFamily: "monospace" }}>
                    {formatCurrency(sonnetCost.totalCost)}
                  </Table.Td>
                </Table.Tr>
              ) : null}
              {opusInputTokens > 0 || opusOutputTokens > 0 ? (
                <Table.Tr>
                  <Table.Td>
                    <Badge size="sm" variant="light" color="violet">
                      Opus 4.5
                    </Badge>
                  </Table.Td>
                  <Table.Td style={{ textAlign: "right", fontFamily: "monospace" }}>
                    {formatTokenCount(opusInputTokens)}
                  </Table.Td>
                  <Table.Td style={{ textAlign: "right", fontFamily: "monospace" }}>
                    {formatTokenCount(opusOutputTokens)}
                  </Table.Td>
                  <Table.Td style={{ textAlign: "right", fontFamily: "monospace" }}>
                    {formatCurrency(opusCost.totalCost)}
                  </Table.Td>
                </Table.Tr>
              ) : null}
            </Table.Tbody>
          </Table>
        </Stack>
      )}

      {/* Model Comparison - Always show for reference */}
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
              {Object.entries(MODEL_PRICING)
                .filter(([model]) => model.includes("4.5")) // Only show 4.5 models
                .map(([model, pricing]) => {
                  const cost = calculateCost(model, inputTotal, outputTotal);
                  return (
                    <Table.Tr key={model}>
                      <Table.Td>
                        <Group gap="xs">
                          <Badge size="xs" variant="light" color={pricing.color}>
                            {model.replace("claude-", "").replace("-", " ")}
                          </Badge>
                        </Group>
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
