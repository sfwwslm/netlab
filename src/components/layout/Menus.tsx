import React, { useEffect, useRef, useState, useCallback } from "react";
import { useNavigate, useLocation } from "react-router-dom";
import { getMenuItems, MenuItem } from "@/constants/menuItems";
import styled from "styled-components";
import { useTranslation } from "react-i18next";
import { useModal } from "@/contexts/ModalContext";
import SettingsModal from "@/features/Settings/SettingsModal";
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";

const MenuBar = styled.nav`
  border: 1px solid ${(props) => props.theme.colors.header.border};
  border-radius: 5px;
  height: 100%;
`;

const MenuList = styled.ul`
  display: flex;
  list-style: none;
  margin: 0;
  padding: 0;
  height: 100%;
`;

const MenuItemStyled = styled.li<{ $isActive: boolean }>`
  cursor: pointer;
  position: relative;
  user-select: none;
  display: flex;
  justify-content: center;
  align-items: center;
  height: 100%;
  min-width: 30px;
  margin: 0 5px;
  color: ${(props) => props.theme.colors.header.text};

  &:hover {
    background-color: ${(props) => props.theme.colors.header.hoverBackground};
    box-shadow: 0 0 8px ${(props) => props.theme.colors.header.hoverShadow};
  }

  &.active {
    font-weight: bold;
    color: ${(props) => props.theme.colors.primary};
  }

  span {
    padding: 0 3px;
  }
`;

const SubmenuContainer = styled.ul`
  position: absolute; /* 绝对定位，相对于其父元素 li */
  top: 100%; /* 定位到父元素底部 */
  left: 0; /* 从父元素的左侧开始 */
  list-style: none; /* 移除列表默认的项目符号 */
  padding: 0; /* 移除默认内边距 */
  border: 1px solid ${(props) => props.theme.colors.header.border};
  border-radius: ${(props) => props.theme.radii.small};
  color: ${(props) => props.theme.colors.textPrimary};
  z-index: 1000; /* 确保子菜单位于其他内容之上 */
  background-color: ${(props) => props.theme.colors.background};
  min-width: max-content; /* 宽度适应内容，不换行 */
  white-space: nowrap; /* 防止内容换行 */
`;

const SubmenuItemStyled = styled.li<{ $isActive: boolean }>`
  margin: 5px;
  font-size: 0.8em;
  border: 1px solid ${(props) => props.theme.colors.header.border};
  border-radius: ${(props) => props.theme.radii.small};
  padding: 5px 15px;
  cursor: pointer;
  color: ${(props) => props.theme.colors.textPrimary};

  &:hover {
    background-color: ${(props) => props.theme.colors.header.hoverBackground};
    box-shadow: 0 0 8px ${(props) => props.theme.colors.header.hoverShadow};
  }

  &.active {
    font-weight: bold;
    color: ${(props) => props.theme.colors.primary};
  }
`;

/** Submenu 组件：负责渲染子菜单 */
interface SubmenuProps {
  items: MenuItem[]; // 子菜单项数组
  currentPath: string; // 当前路由路径，用于判断激活状态
  onItemClick: (item: MenuItem) => void; // 子菜单项点击回调
}

const Submenu: React.FC<SubmenuProps> = React.memo(
  ({ items, currentPath, onItemClick }) => {
    return (
      <SubmenuContainer>
        {items.map((sub) => (
          <SubmenuItemStyled
            key={sub.id}
            $isActive={currentPath === sub.url}
            onClick={(e) => {
              e.stopPropagation(); // 阻止事件冒泡到父菜单项，避免不必要的父菜单点击逻辑
              onItemClick(sub); // 调用父组件传入的点击回调
            }}
          >
            <span>{sub.label}</span>
          </SubmenuItemStyled>
        ))}
      </SubmenuContainer>
    );
  },
);
Submenu.displayName = "Submenu"; // 便于 React DevTools 识别，作用是为了方便调试

const Menus: React.FC = () => {
  const { openModal } = useModal();
  /**
   * 追踪当前哪个父菜单项的子菜单是激活（即显示）状态。
   * 值为父菜单项的 ID，如果没有任何子菜单被激活，则为 null。
   */
  const [activeSubmenuId, setActiveSubmenuId] = useState<number | null>(null);
  /**
   * 控制菜单是否进入“悬停即展开”模式。
   * 一旦用户首次点击了带有子菜单的父菜单项，此状态变为 true。
   */
  const [isHoverMode, setIsHoverMode] = useState(false);
  /**
   * 在悬停模式下，记录最后一个被鼠标悬停过的、且拥有子菜单的父菜单项ID。
   * 用于在鼠标移动到没有子菜单的项上时，保持之前子菜单的开启状态。
   */
  const [lastHoveredWithChildrenId, setLastHoveredWithChildrenId] = useState<
    number | null
  >(null);

  const menuRef = useRef<HTMLDivElement>(null);
  const navigate = useNavigate();
  const location = useLocation();
  const { t } = useTranslation();
  const currentPath = location.pathname;

  /**
   * 关闭所有子菜单并退出悬停模式。
   *
   * 此函数被多个事件（点击子菜单项、导航、外部点击、二次点击父菜单项）调用。
   */
  const closeAllSubmenus = useCallback(() => {
    setActiveSubmenuId(null); // 清除当前激活的子菜单ID
    setIsHoverMode(false); // 退出悬停模式
    setLastHoveredWithChildrenId(null); // 清除最后悬停的带子菜单的ID
  }, []);

  // 通过监听全局点击事件，实现“点击菜单外部关闭子菜单”的功能
  useEffect(() => {
    const handleGlobalClick = (event: MouseEvent) => {
      // 只有当处于“悬停模式”时，全局点击才可能触发关闭。
      if (isHoverMode) {
        // 检查点击是否发生在菜单栏容器（menuRef.current）之外。
        if (
          menuRef.current &&
          !menuRef.current.contains(event.target as Node)
        ) {
          closeAllSubmenus();
        }
      }
    };

    document.addEventListener("mousedown", handleGlobalClick);

    // 清理函数：组件卸载时移除事件监听器，防止内存泄漏。
    return () => {
      document.removeEventListener("mousedown", handleGlobalClick);
    };
  }, [isHoverMode, closeAllSubmenus]);

  const openLogWindow = useCallback(() => {
    void (async () => {
      try {
        const existing = await WebviewWindow.getByLabel("log-window");
        if (existing) {
          await existing.setDecorations(false);
          await existing.show();
          await existing.setFocus();
          return;
        }
        const logWindow = new WebviewWindow("log-window", {
          url: "/logs",
          title: t("logWindow.title"),
          width: 980,
          height: 640,
          minWidth: 820,
          minHeight: 520,
          resizable: true,
          decorations: false,
          transparent: true,
          visible: true,
          center: true,
        });
        logWindow.once("tauri://created", async () => {
          await logWindow.setDecorations(false);
          await logWindow.show();
          await logWindow.setFocus();
        });
      } catch (error) {
        console.error("Failed to open log window:", error);
      }
    })();
  }, [t]);

  /**
   * 处理主菜单项的点击逻辑
   *
   * @param item 被点击的 MenuItem 对象
   */
  const handleMenuItemClick = useCallback(
    (item: MenuItem) => {
      if (item.children) {
        if (isHoverMode) {
          // 在悬停模式下，重复点击同一个父菜单项会关闭其子菜单
          if (activeSubmenuId === item.id) {
            closeAllSubmenus();
          } else {
            // 点击另一个父菜单项则切换子菜单
            setActiveSubmenuId(item.id);
            setLastHoveredWithChildrenId(item.id);
          }
        } else {
          // 首次点击带子菜单的项，会进入悬停模式并打开其子菜单
          setActiveSubmenuId(item.id);
          setIsHoverMode(true);
          setLastHoveredWithChildrenId(item.id);
        }
      } else if (item.url === "/logs") {
        openLogWindow();
        closeAllSubmenus();
      } else {
        // 点击没有子菜单的项（叶子节点），则直接导航
        navigate(item.url);
        closeAllSubmenus();
      }
    },
    [navigate, activeSubmenuId, isHoverMode, closeAllSubmenus, openLogWindow],
  );

  /**
   * 处理子菜单项的点击逻辑
   *
   * @param sub 被点击的子菜单项对象
   */
  const handleSubmenuItemClick = useCallback(
    (sub: MenuItem) => {
      if (sub.url === "/help/settings") {
        openModal((close) => <SettingsModal onClose={close} />, {
          key: "settings",
        }); // 特殊处理：打开设置模态框
      } else {
        navigate(sub.url);
      }
      closeAllSubmenus();
    },
    [navigate, closeAllSubmenus, openModal],
  );

  /**
   * 在悬停模式下，处理鼠标进入菜单项的逻辑
   *
   * @param itemId 鼠标进入的菜单项的 ID
   */
  const handleMouseEnter = useCallback(
    (itemId: number) => {
      if (isHoverMode) {
        const hoveredItem = menuItems.find((menu) => menu.id === itemId);
        if (hoveredItem?.children) {
          // 如果悬停在有子菜单的项上，则显示其子菜单
          setActiveSubmenuId(itemId);
          setLastHoveredWithChildrenId(itemId);
        } else {
          // 如果悬停在没有子菜单的项上，则保持上一个有子菜单的项的子菜单打开状态
          setActiveSubmenuId(lastHoveredWithChildrenId);
        }
      }
    },
    [isHoverMode, lastHoveredWithChildrenId],
  );

  const menuItems = getMenuItems(t);

  return (
    <MenuBar ref={menuRef}>
      <MenuList>
        {menuItems.map((item) => {
          const isActive =
            currentPath === item.url ||
            item.children?.some((sub) => sub.url === currentPath) ||
            false;
          const isSubmenuOpen = activeSubmenuId === item.id;

          return (
            <MenuItemStyled
              key={item.id}
              $isActive={isActive}
              onClick={() => handleMenuItemClick(item)}
              onMouseEnter={() => handleMouseEnter(item.id)}
            >
              <span>{item.label}</span>
              {item.children && isSubmenuOpen && (
                <Submenu
                  items={item.children}
                  currentPath={currentPath}
                  onItemClick={handleSubmenuItemClick}
                />
              )}
            </MenuItemStyled>
          );
        })}
      </MenuList>
    </MenuBar>
  );
};

export default Menus;
