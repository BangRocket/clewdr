import React from "react";
import { ActionIcon, Loader, Tooltip } from "@mantine/core";
import { IconTrash } from "@tabler/icons-react";

interface DeleteButtonProps {
  cookie: string;
  onDelete: (cookie: string) => void;
  isDeleting: boolean;
}

const DeleteButton: React.FC<DeleteButtonProps> = ({
  cookie,
  onDelete,
  isDeleting,
}) => {
  return (
    <Tooltip label="Delete cookie">
      <ActionIcon
        variant="subtle"
        color="red"
        size="sm"
        onClick={() => onDelete(cookie)}
        disabled={isDeleting}
        ml="xs"
      >
        {isDeleting ? <Loader size={14} /> : <IconTrash size={14} />}
      </ActionIcon>
    </Tooltip>
  );
};

export default DeleteButton;
