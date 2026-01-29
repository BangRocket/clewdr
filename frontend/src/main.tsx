// frontend/src/main.tsx
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { MantineProvider } from "@mantine/core";
import { Notifications } from "@mantine/notifications";
import "@mantine/core/styles.css";
import "@mantine/notifications/styles.css";
import "@mantine/code-highlight/styles.css";
import "./index.css";
import "./i18n";
import App from "./App.tsx";
import { AppProvider } from "./context/AppContext";
import { theme } from "./theme";

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <MantineProvider theme={theme} defaultColorScheme="dark">
      <Notifications position="top-right" />
      <AppProvider>
        <App />
      </AppProvider>
    </MantineProvider>
  </StrictMode>
);
