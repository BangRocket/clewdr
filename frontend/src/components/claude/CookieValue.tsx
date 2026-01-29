import React, { useState } from "react";
import { useTranslation } from "react-i18next";
import { Group, Code, ActionIcon, Tooltip, UnstyledButton, Box } from "@mantine/core";
import { IconChevronUp, IconChevronDown, IconCopy } from "@tabler/icons-react";
import { notifications } from "@mantine/notifications";

interface CookieValueProps {
  cookie: string;
}

const CookieValue: React.FC<CookieValueProps> = ({ cookie }) => {
  const { t } = useTranslation();
  const [isExpanded, setIsExpanded] = useState(false);

  if (!cookie) return null;

  // Clean cookie value for display
  const cleanCookie = cookie.replace(/sessionKey=sk-ant-sid01-/, "");
  const displayText = isExpanded
    ? cleanCookie
    : `${cleanCookie.substring(0, 30)}${cleanCookie.length > 30 ? "..." : ""}`;

  const copyToClipboard = (text: string, event: React.MouseEvent) => {
    event.stopPropagation();
    navigator.clipboard
      .writeText(text)
      .then(() => {
        notifications.show({
          message: t("common.copy"),
          color: "green",
        });
      })
      .catch((err) => console.error("Failed to copy: ", err));
  };

  return (
    <Group gap="xs" wrap="nowrap" style={{ minWidth: 0 }}>
      <UnstyledButton
        onClick={() => setIsExpanded(!isExpanded)}
        style={{ flex: 1, minWidth: 0 }}
      >
        <Group gap="xs" wrap="nowrap">
          <Code
            style={{
              wordBreak: isExpanded ? "break-all" : undefined,
              overflow: isExpanded ? "visible" : "hidden",
              textOverflow: isExpanded ? undefined : "ellipsis",
              whiteSpace: isExpanded ? "normal" : "nowrap",
              background: "transparent",
              padding: 0,
            }}
          >
            {displayText}
          </Code>
          {cleanCookie.length > 30 && (
            <Box c="dimmed" style={{ flexShrink: 0 }}>
              {isExpanded ? (
                <IconChevronUp size={14} />
              ) : (
                <IconChevronDown size={14} />
              )}
            </Box>
          )}
        </Group>
      </UnstyledButton>
      <Tooltip label={t("cookieStatus.copy")}>
        <ActionIcon
          variant="subtle"
          color="gray"
          size="sm"
          onClick={(e) => copyToClipboard(cleanCookie, e)}
        >
          <IconCopy size={14} />
        </ActionIcon>
      </Tooltip>
    </Group>
  );
};

export default CookieValue;
