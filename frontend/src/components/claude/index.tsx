// frontend/src/components/claude/index.tsx
import { useTranslation } from "react-i18next";
import { Stack, Button, Modal, Title, Group } from "@mantine/core";
import { useDisclosure } from "@mantine/hooks";
import { IconPlus } from "@tabler/icons-react";
import CookieSubmitForm from "./CookieSubmitForm";
import CookieVisualization from "./CookieVisualization";

export function CookiesPage() {
  const { t } = useTranslation();
  const [opened, { open, close }] = useDisclosure(false);

  return (
    <>
      <Stack gap="md">
        <Group justify="flex-end">
          <Button
            leftSection={<IconPlus size={16} />}
            onClick={open}
            variant="gradient"
            gradient={{ from: "cyan", to: "violet", deg: 90 }}
          >
            {t("claudeTab.submit")}
          </Button>
        </Group>

        <CookieVisualization />
      </Stack>

      <Modal
        opened={opened}
        onClose={close}
        title={<Title order={4}>{t("claudeTab.submit")}</Title>}
        size="lg"
        centered
      >
        <CookieSubmitForm onSuccess={close} />
      </Modal>
    </>
  );
}

export default CookiesPage;
