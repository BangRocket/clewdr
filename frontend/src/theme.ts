// frontend/src/theme.ts
import { createTheme, MantineColorsTuple } from "@mantine/core";

// Custom cyan color for the app accent
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

export const theme = createTheme({
  primaryColor: "cyan",
  colors: {
    cyan,
    violet,
  },
  fontFamily:
    "system-ui, -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, 'Helvetica Neue', Arial, sans-serif",
  headings: {
    fontFamily:
      "system-ui, -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, 'Helvetica Neue', Arial, sans-serif",
  },
  defaultRadius: "md",
  cursorType: "pointer",
  components: {
    Button: {
      defaultProps: {
        variant: "filled",
      },
    },
    Card: {
      defaultProps: {
        withBorder: true,
      },
    },
    Paper: {
      defaultProps: {
        withBorder: true,
      },
    },
  },
});
