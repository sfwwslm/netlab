import { GlobalStyle } from "@/styles/GlobalStyles";
import React, { useEffect } from "react";
import { Outlet, useLocation } from "react-router-dom";
import Menus from "@/components/layout/Menus";
import WindowControls from "@/components/layout/WindowControls";
import dragging from "@/utils/dragging";
import { debug, attachConsole } from "@tauri-apps/plugin-log";
import styled from "styled-components";
import { AppProvider } from "@/contexts/AppProvider";
import AppEventManager from "@/components/layout/AppEventManager";
import { useDisableBrowserShortcuts } from "@/hooks/useDisableBrowserShortcuts";

const AppContainer = styled.div`
  display: flex;
  flex-direction: column;
  height: 100vh; /* 确保占据整个视口高度 */
`;

const AppHeader = styled.header`
  display: flex;
  justify-content: space-between;
  align-items: center;
  flex-shrink: 0;
  height: ${(props) => props.theme.sizing.appHeaderHeight};
  background-color: ${(props) => props.theme.colors.background};
  color: ${(props) => props.theme.colors.textPrimary};
  user-select: none;
  z-index: ${(props) => props.theme.zIndices.appHeader};
`;

const HeaderLeft = styled.div`
  height: 100%;
  margin-left: ${(props) => props.theme.spacing.headerMargin};
  font-size: ${(props) => props.theme.typography.menuFontSize};
`;

const HeaderRight = styled.div`
  height: 100%;
  margin-right: ${(props) => props.theme.spacing.headerMargin};
  // 使用 flex 布局来横向排列内部组件
  display: flex;
  align-items: center;
  gap: 5px; /* 在用户组件和窗口控制按钮之间添加一些间距 */
`;

const AppMain = styled.main`
  background-color: ${(props) => props.theme.colors.background};
  flex: 1;
  overflow: auto;
  margin: 0;
`;

const AppLayout: React.FC = () => {
  const location = useLocation();

  useDisableBrowserShortcuts();

  useEffect(() => {
    debug(`路由切换到：${location.pathname}`);
  }, [location]);

  useEffect(() => {
    if (import.meta.env.DEV) {
      attachConsole();
    }
  }, []);

  return (
    <AppContainer className="app-container">
      <AppHeader className="app-header" onMouseDown={dragging}>
        <HeaderLeft className="header-left">
          <Menus />
        </HeaderLeft>
        <HeaderRight className="header-right">
          <WindowControls />
        </HeaderRight>
      </AppHeader>
      <AppMain className="app-main">
        <Outlet />
      </AppMain>
    </AppContainer>
  );
};

export default function App() {
  return (
    <AppProvider>
      <GlobalStyle />
      {/* 在这里调用事件管理器，它会在整个应用生命周期中存在 */}
      <AppEventManager />
      <AppLayout />
    </AppProvider>
  );
}
