// frontend/src/theme.ts
import { createTheme, MantineColorsTuple } from "@mantine/core";

// Custom cyan/teal color for primary accent (matching CRM dashboard)
const cyan: MantineColorsTuple = [
  "#e0fcff",
  "#b8f2fc",
  "#8de8f8",
  "#5edef5",
  "#22d3ee", // Main cyan-400
  "#06b6d4", // cyan-500
  "#0891b2",
  "#0e7490",
  "#155e75",
  "#164e63",
];

// Custom violet/purple accent
const violet: MantineColorsTuple = [
  "#f5f3ff",
  "#ede9fe",
  "#ddd6fe",
  "#c4b5fd",
  "#a78bfa", // violet-400
  "#8b5cf6", // violet-500
  "#7c3aed",
  "#6d28d9",
  "#5b21b6",
  "#4c1d95",
];

// Magenta/pink accent for secondary highlights
const magenta: MantineColorsTuple = [
  "#fdf2f8",
  "#fce7f3",
  "#fbcfe8",
  "#f9a8d4",
  "#f472b6", // pink-400
  "#ec4899", // pink-500
  "#db2777",
  "#be185d",
  "#9d174d",
  "#831843",
];

// Dark navy colors for backgrounds
const navy: MantineColorsTuple = [
  "#e8eaf0",
  "#c4c9d4",
  "#9ca3b4",
  "#6b7394",
  "#4a5276",
  "#2d3354", // sidebar bg
  "#1e2442", // main bg
  "#171c35", // darker areas
  "#10142a",
  "#0a0d1f", // darkest
];

export const theme = createTheme({
  primaryColor: "cyan",
  colors: {
    cyan,
    violet,
    magenta,
    navy,
  },
  fontFamily:
    "system-ui, -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, 'Helvetica Neue', Arial, sans-serif",
  headings: {
    fontFamily:
      "system-ui, -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, 'Helvetica Neue', Arial, sans-serif",
  },
  defaultRadius: "md",
  cursorType: "pointer",
  other: {
    // Custom colors for the CRM-style dashboard
    sidebarBg: "#1a1f37",
    mainBg: "#0f1225",
    cardBg: "rgba(26, 31, 55, 0.8)",
    cardBorder: "rgba(255, 255, 255, 0.08)",
    glassOverlay: "rgba(255, 255, 255, 0.03)",
  },
  components: {
    Button: {
      defaultProps: {
        variant: "filled",
      },
    },
    Card: {
      defaultProps: {
        withBorder: false,
      },
    },
    Paper: {
      defaultProps: {
        withBorder: false,
      },
    },
  },
});
