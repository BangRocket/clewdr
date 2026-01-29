// frontend/src/components/console/index.tsx
import { Box } from "@mantine/core";
import LogsPanel from "../dashboard/LogsPanel";

export function LogsTab() {
  return (
    <Box h="calc(100vh - 200px)">
      <LogsPanel />
    </Box>
  );
}

export default LogsTab;
