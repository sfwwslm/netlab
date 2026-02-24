import { useEffect, useState, useCallback } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import styled from "styled-components";
import { MdWbSunny, MdOutlineWbSunny } from "react-icons/md";
import {
  VscChromeMinimize,
  VscChromeMaximize,
  VscChromeRestore,
  VscChromeClose,
} from "react-icons/vsc";
import { useAppTheme } from "@/contexts/ThemeContext";
import { getMinimizeToTrayOnClose, getStartMinimized } from "@/utils/config";

const ControlsContainer = styled.nav`
  display: flex;
  height: 100%;
  border: 1px solid ${(props) => props.theme.colors.header.border};
  border-radius: 5px;
  align-items: center;
`;

const ControlButtonsList = styled.ul`
  display: flex;
  list-style: none;
  margin: 0;
  padding: 0;
  align-items: center;
  height: 100%; // 确保列表项垂直居中
`;

const ControlButton = styled.li`
  display: flex;
  height: 100%;
  min-width: 30px;
  justify-content: center;
  align-items: center;
  cursor: pointer;
  position: relative;
  user-select: none;
  font-size: 1.2rem;
  color: ${(props) => props.theme.colors.textPrimary};

  &:hover {
    background-color: ${(props) => props.theme.colors.header.hoverBackground};
    box-shadow: 0 0 8px ${(props) => props.theme.colors.header.hoverShadow};
  }
`;

const ThemeSwitchButton = styled(ControlButton)``;

export default function WindowControls() {
  const { toggleTheme, themeName } = useAppTheme();
  const [isWindowMaximized, setIsWindowMaximized] = useState(false);

  const handleToggleMaximize = useCallback(async () => {
    const currentWindow = getCurrentWindow();
    await currentWindow.toggleMaximize();
    const maximized = await currentWindow.isMaximized();
    setIsWindowMaximized(maximized);
  }, []);

  const handleMinimize = useCallback(() => {
    getCurrentWindow().minimize();
  }, []);

  /**
   * @function handleClose
   * @description 根据配置决定是隐藏窗口还是直接关闭
   */
  const handleClose = useCallback(async () => {
    // 隐藏窗口到托盘，而不是关闭窗口
    if (await getMinimizeToTrayOnClose()) {
      await getCurrentWindow().hide();
    } else {
      // 如果设置了启动时最小化，则先隐藏再关闭窗口
      if (await getStartMinimized()) {
        await getCurrentWindow().hide();
      }
      await getCurrentWindow().close();
    }
  }, []);

  useEffect(() => {
    const currentWindow = getCurrentWindow();
    let unlistenResized: (() => void) | undefined;

    const setupWindowListeners = async () => {
      const maximized = await currentWindow.isMaximized();
      setIsWindowMaximized(maximized);

      unlistenResized = await currentWindow.onResized(async () => {
        const isCurrentlyMaximized = await currentWindow.isMaximized();
        setIsWindowMaximized(isCurrentlyMaximized);
      });
    };

    setupWindowListeners();

    return () => {
      unlistenResized?.();
    };
  }, []);

  return (
    <ControlsContainer>
      <ControlButtonsList>
        <ThemeSwitchButton onClick={toggleTheme} aria-label="Toggle theme">
          {themeName === "light" ? <MdOutlineWbSunny /> : <MdWbSunny />}
        </ThemeSwitchButton>
        <ControlButton
          id="window-minimize"
          onClick={handleMinimize}
          aria-label="Minimize window"
        >
          <VscChromeMinimize />
        </ControlButton>
        <ControlButton
          id="window-maximize"
          onClick={handleToggleMaximize}
          aria-label={isWindowMaximized ? "Restore window" : "Maximize window"}
        >
          {isWindowMaximized ? <VscChromeRestore /> : <VscChromeMaximize />}
        </ControlButton>
        <ControlButton
          id="window-close"
          onClick={handleClose}
          aria-label="Close window"
        >
          <VscChromeClose />
        </ControlButton>
      </ControlButtonsList>
    </ControlsContainer>
  );
}
