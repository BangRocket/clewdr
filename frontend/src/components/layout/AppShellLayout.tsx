// frontend/src/components/layout/AppShellLayout.tsx
import { ReactNode } from "react";
import { useTranslation } from "react-i18next";
import {
  AppShell,
  Burger,
  Group,
  Text,
  useMantineColorScheme,
  ActionIcon,
  Tooltip,
  Box,
  Anchor,
  Stack,
  UnstyledButton,
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
  IconLayoutDashboard,
  IconDatabase,
  IconActivity,
  IconUser,
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
}

interface NavSection {
  titleKey: string;
  icon: typeof IconLayoutDashboard;
  items: NavItem[];
}

const navSections: NavSection[] = [
  {
    titleKey: "sidebar.sections.overview",
    icon: IconLayoutDashboard,
    items: [
      { id: "dashboard", labelKey: "sidebar.dashboard", icon: IconChartBar },
    ],
  },
  {
    titleKey: "sidebar.sections.management",
    icon: IconDatabase,
    items: [
      { id: "cookies", labelKey: "sidebar.cookies", icon: IconCookie },
      { id: "config", labelKey: "sidebar.config", icon: IconSettings },
    ],
  },
  {
    titleKey: "sidebar.sections.monitoring",
    icon: IconActivity,
    items: [
      { id: "logs", labelKey: "sidebar.logs", icon: IconTerminal2 },
    ],
  },
  {
    titleKey: "sidebar.sections.account",
    icon: IconUser,
    items: [
      { id: "auth", labelKey: "sidebar.auth", icon: IconKey },
    ],
  },
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
        width: 240,
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
              color="white"
            />
            <Text
              component="h1"
              size="xl"
              fw={700}
              className="gradient-text"
            >
              {t("app.title")}
            </Text>
          </Group>
          <Group gap="xs">
            <Tooltip label={t("settings.language")}>
              <ActionIcon
                variant="subtle"
                color="gray"
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
              <ActionIcon variant="subtle" color="gray" onClick={() => toggleColorScheme()}>
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

      <AppShell.Navbar p={0}>
        <Stack gap={0} style={{ height: "100%" }}>
          {/* Navigation Sections */}
          <Box style={{ flex: 1, overflowY: "auto" }} py="md">
            {navSections.map((section) => (
              <Box key={section.titleKey} mb="sm">
                {/* Section Header */}
                <Box className="nav-section-header">
                  <section.icon size={14} />
                  <span>{t(section.titleKey)}</span>
                </Box>

                {/* Section Items */}
                {section.items.map((item) => (
                  <UnstyledButton
                    key={item.id}
                    className={`nav-item ${activeSection === item.id ? "active" : ""}`}
                    onClick={() => handleNavClick(item.id)}
                    w="100%"
                  >
                    <Group gap="sm">
                      <item.icon size={18} style={{ opacity: 0.8 }} />
                      <Text size="sm">{t(item.labelKey)}</Text>
                    </Group>
                  </UnstyledButton>
                ))}
              </Box>
            ))}
          </Box>

          {/* Footer */}
          <Box
            py="md"
            px="md"
            style={{ borderTop: "1px solid var(--card-border)" }}
          >
            <Text size="xs" c="dimmed" ta="center">
              {t("sidebar.version")}
            </Text>
          </Box>
        </Stack>
      </AppShell.Navbar>

      <AppShell.Main>
        <Box mih="calc(100vh - 60px - 32px - 50px)">{children}</Box>
        <Box
          component="footer"
          py="md"
          mt="md"
          style={{ borderTop: "1px solid var(--card-border)" }}
        >
          <Group justify="center" gap="xs">
            <Text size="sm" c="dimmed">
              {version}
            </Text>
            <Text size="sm" c="dimmed">
              |
            </Text>
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
