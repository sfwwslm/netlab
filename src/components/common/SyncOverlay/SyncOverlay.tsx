import React from "react";
import {
  Overlay,
  Spinner,
  LoadingText,
} from "../LoadingOverlay/LoadingOverlay.styles";
import { useTheme } from "styled-components";
import { StyledButton } from "@/components/styled/StyledButton";

interface SyncOverlayProps {
  isOpen: boolean;
  text: string;
  completed: boolean;
  onConfirm: () => void;
}

const SyncOverlay: React.FC<SyncOverlayProps> = ({
  isOpen,
  text,
  completed,
  onConfirm,
}) => {
  const theme = useTheme();

  if (!isOpen) return null;

  return (
    <Overlay
      className="sync-overlay"
      initial={{ opacity: 0 }}
      animate={{ opacity: 1 }}
      exit={{ opacity: 0 }}
      // 将 top 设置为 0，覆盖整个窗口
      style={{
        zIndex: theme.zIndices.appHeader + 1, // 确保覆盖 AppHeader
        top: 0,
        background: "rgba(0,0,0,0.7)",
      }}
    >
      {/* 完成后隐藏 Spinner */}
      {!completed && <Spinner className="sync-spinner" />}
      <LoadingText className="sync-text">{text}</LoadingText>
      {/* 完成后显示确认按钮 */}
      {completed && (
        <StyledButton
          variant="primary"
          onClick={onConfirm}
          style={{ marginTop: "20px" }}
        >
          确认
        </StyledButton>
      )}
    </Overlay>
  );
};

export default SyncOverlay;
