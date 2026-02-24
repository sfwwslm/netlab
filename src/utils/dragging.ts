import { getCurrentWindow } from "@tauri-apps/api/window";

/**
 * 窗口拖动函数
 * @param event
 * @returns
 */
async function dragging(
  event: React.MouseEvent<HTMLDivElement, MouseEvent>,
): Promise<void> {
  // 确保只在 Header 本身触发拖动，而不是在子元素上
  if (event.target !== event.currentTarget) {
    return; // 如果点击的是子元素，直接返回
  }

  /// 鼠标左键事件
  if (event.buttons === 1) {
    // 左键单击 - 拖动窗口
    if (event.detail === 1) {
      const isMaximized = await getCurrentWindow().isMaximized();
      if (!isMaximized) {
        await getCurrentWindow().startDragging();
      }
    } else {
      /// 左键多击 - 切换窗口大小（最大化或恢复原大小）
      await getCurrentWindow().toggleMaximize();
    }
  }
}

export default dragging;
