import React from "react";
import { Overlay, Spinner, LoadingText } from "./LoadingOverlay.styles";

interface LoadingOverlayProps {
  isOpen: boolean;
  text?: string;
}

/**
 * @component LoadingOverlay
 * @description 一个通用的加载遮罩层组件，用于覆盖在某个容器上显示加载状态。
 * @param {boolean} isOpen - 控制遮罩层的显示与隐藏。
 * @param {string} [text="正在加载中..."] - 加载时显示的文本。
 */
const LoadingOverlay: React.FC<LoadingOverlayProps> = ({
  isOpen,
  text = "正在加载中...",
}) => {
  if (!isOpen) return null;

  return (
    <Overlay
      className="loading-overlay"
      initial={{ opacity: 0 }}
      animate={{ opacity: 1 }}
      exit={{ opacity: 0 }}
    >
      <Spinner className="loading-spinner" />
      <LoadingText className="loading-text">{text}</LoadingText>
    </Overlay>
  );
};

export default LoadingOverlay;
