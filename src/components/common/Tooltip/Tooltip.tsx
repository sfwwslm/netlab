import React, { useState, useRef, ReactNode, useLayoutEffect } from "react";
import { createPortal } from "react-dom";
import { TooltipContainer, TooltipText } from "./Tooltip.styles";

// createPortal 可以将一个组件的渲染内容“传送”到 DOM 树的另一个位置。让它的提示文本直接被渲染到 document.body 的顶层，而不是在当前组件树的位置。这样一来，无论提示文本如何显示或隐藏，它都与模态框的布局完全隔离，从而彻底消除抖动问题。

/**
 * @interface TooltipProps
 * @description Tooltip 组件的 props 定义
 * @property {ReactNode} children - 触发工具提示的子元素
 * @property {string} text - 在工具提示中显示的文本
 */
interface TooltipProps {
  children: ReactNode;
  text: string;
}

/**
 * @interface Position
 * @description 定义位置坐标
 */
interface Position {
  top: number;
  left: number;
}

/**
 * @component Tooltip
 * @description 一个通用的鼠标悬浮气泡文本展示组件。它只在需要时渲染，并动态计算位置，以防止任何布局抖动或溢出。
 */
const Tooltip: React.FC<TooltipProps> = ({ children, text }) => {
  const [isVisible, setIsVisible] = useState(false);
  const [position, setPosition] = useState<Position | null>(null);
  const wrapperRef = useRef<HTMLDivElement>(null);
  const tooltipRef = useRef<HTMLDivElement>(null);

  // 使用 useLayoutEffect 可以在 DOM 更新后立即同步执行，防止闪烁
  useLayoutEffect(() => {
    // 仅在 isVisible 变为 true 且 DOM 元素可用时计算
    if (isVisible && wrapperRef.current && tooltipRef.current) {
      const wrapperRect = wrapperRef.current.getBoundingClientRect();
      const tooltipRect = tooltipRef.current.getBoundingClientRect();

      // 1. 计算理想的 top 和 left 值 (在 wrapper 上方居中)
      let top = wrapperRect.top - tooltipRect.height - 8;
      let left = wrapperRect.left + (wrapperRect.width - tooltipRect.width) / 2;

      // 2. 检查并修正水平方向的溢出
      const margin = 5; // 距离视口边缘的最小间距
      if (left < margin) {
        left = margin;
      } else if (left + tooltipRect.width > window.innerWidth - margin) {
        left = window.innerWidth - tooltipRect.width - margin;
      }

      // 3. 检查并修正垂直方向的溢出 (如果上方空间不足，则显示在下方)
      if (top < margin) {
        top = wrapperRect.bottom + 8; // 移动到下方
      }

      setPosition({ top, left });
    }
  }, [isVisible, text]); // 当 isVisible 或 text 变化时重新计算

  return (
    <TooltipContainer
      ref={wrapperRef}
      onMouseEnter={() => setIsVisible(true)}
      onMouseLeave={() => setIsVisible(false)}
      className="tooltip-wrapper"
    >
      {children}
      {/* 使用 createPortal 将 TooltipText 渲染到 document.body */}
      {isVisible &&
        createPortal(
          <TooltipText
            ref={tooltipRef}
            // 仅在 position 计算出来后应用样式
            style={
              position
                ? { top: `${position.top}px`, left: `${position.left}px` }
                : {}
            }
            className="tooltip-text"
          >
            {text}
          </TooltipText>,
          document.body, // 指定传送的目标 DOM 节点
        )}
    </TooltipContainer>
  );
};

export default Tooltip;
