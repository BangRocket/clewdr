// frontend/src/components/layout/AppShellLayout.tsx
import { ReactNode } from "react";
import { useTranslation } from "react-i18next";
import {
  AppShell,
  Burger,
  Group,
  NavLink,
  Text,
  useMantineColorScheme,
  ActionIcon,
  Tooltip,
  Box,
  Anchor,
  Divider,
} from "@mantine/core";
import { useDisclosure } from "@mantine/hooks";
import {
  IconChartBar,
  IconCookie,
  IconTerminal2,
  IconSettings,
  IconKey,
  IconSun,
  IconMoon,
  IconLanguage,
} from "@tabler/icons-react";

interface AppShellLayoutProps {
  children: ReactNode;
  version: string;
  activeSection: string;
  onSectionChange: (section: string) => void;
}

interface NavItem {
  id: string;
  labelKey: string;
  icon: typeof IconChartBar;
  color: string;
}

const navItems: NavItem[] = [
  { id: "dashboard", labelKey: "sidebar.dashboard", icon: IconChartBar, color: "cyan" },
  { id: "cookies", labelKey: "sidebar.cookies", icon: IconCookie, color: "yellow" },
  { id: "logs", labelKey: "sidebar.logs", icon: IconTerminal2, color: "violet" },
  { id: "config", labelKey: "sidebar.config", icon: IconSettings, color: "green" },
  { id: "auth", labelKey: "sidebar.auth", icon: IconKey, color: "grape" },
];

export function AppShellLayout({
  children,
  version,
  activeSection,
  onSectionChange,
}: AppShellLayoutProps) {
  const { t, i18n } = useTranslation();
  const [mobileOpened, { toggle: toggleMobile, close: closeMobile }] = useDisclosure();
  const { colorScheme, toggleColorScheme } = useMantineColorScheme();

  const handleNavClick = (id: string) => {
    onSectionChange(id);
    closeMobile();
  };

  return (
    <AppShell
      header={{ height: 60 }}
      navbar={{
        width: 260,
        breakpoint: "sm",
        collapsed: { mobile: !mobileOpened },
      }}
      padding="md"
    >
      <AppShell.Header>
        <Group h="100%" px="md" justify="space-between">
          <Group>
            <Burger
              opened={mobileOpened}
              onClick={toggleMobile}
              hiddenFrom="sm"
              size="sm"
            />
            <Text
              component="h1"
              size="xl"
              fw={700}
              variant="gradient"
              gradient={{ from: "cyan", to: "violet", deg: 90 }}
            >
              {t("app.title")}
            </Text>
          </Group>
          <Group gap="xs">
            <Text size="sm" c="dimmed" visibleFrom="sm">
              {version}
            </Text>
            <Tooltip label={t("settings.language")}>
              <ActionIcon
                variant="subtle"
                onClick={() => {
                  const nextLang = i18n.language === "en" ? "zh" : "en";
                  i18n.changeLanguage(nextLang);
                }}
              >
                <IconLanguage size={20} />
              </ActionIcon>
            </Tooltip>
            <Tooltip
              label={colorScheme === "dark" ? t("theme.light") : t("theme.dark")}
            >
              <ActionIcon variant="subtle" onClick={() => toggleColorScheme()}>
                {colorScheme === "dark" ? (
                  <IconSun size={20} />
                ) : (
                  <IconMoon size={20} />
                )}
              </ActionIcon>
            </Tooltip>
          </Group>
        </Group>
      </AppShell.Header>

      <AppShell.Navbar p="md">
        <AppShell.Section grow>
          {navItems.map((item) => (
            <NavLink
              key={item.id}
              active={activeSection === item.id}
              label={t(item.labelKey)}
              leftSection={<item.icon size={20} />}
              onClick={() => handleNavClick(item.id)}
              color={item.color}
              variant="light"
              mb={4}
              style={{ borderRadius: "var(--mantine-radius-md)" }}
            />
          ))}
        </AppShell.Section>

        <Divider my="sm" />

        <AppShell.Section>
          <Text size="xs" c="dimmed" ta="center">
            {t("sidebar.version")}
          </Text>
        </AppShell.Section>
      </AppShell.Navbar>

      <AppShell.Main>
        <Box mih="calc(100vh - 60px - 32px - 50px)">{children}</Box>
        <Box
          component="footer"
          py="md"
          mt="md"
          style={{ borderTop: "1px solid var(--mantine-color-default-border)" }}
        >
          <Group justify="center" gap="xs">
            <Text size="sm" c="dimmed">
              {t("app.footer", { year: new Date().getFullYear() })}
            </Text>
            <Text size="sm" c="dimmed">
              |
            </Text>
            <Anchor
              href="https://ko-fi.com/xerxes2"
              target="_blank"
              rel="noopener noreferrer"
              size="sm"
              c="cyan"
            >
              {t("app.buyMeCoffee")}
            </Anchor>
          </Group>
        </Box>
      </AppShell.Main>
    </AppShell>
  );
}

export default AppShellLayout;
