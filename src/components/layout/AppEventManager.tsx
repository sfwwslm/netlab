import { useEffect } from "react";
import { useTranslation } from "react-i18next";
import { emit } from "@tauri-apps/api/event";

/**
 * @component AppEventManager
 * @description 一个无 UI 的组件，专门用于管理应用级的全局事件和副作用。
 * 例如，监听语言变化并通知后端更新托盘菜单。
 */
const AppEventManager: React.FC = () => {
  const { i18n, t } = useTranslation();

  // 监听语言变化，并向后端发送事件以更新系统托盘
  useEffect(() => {
    const updateTrayMenu = () => {
      emit("update-tray-menu", {
        title: t("tray.title"),
        quit: t("tray.quit"),
      });
    };

    // 1. 立即执行一次，以确保应用启动时托盘文本是正确的
    updateTrayMenu();

    // 2. 监听 i18next 的 languageChanged 事件
    i18n.on("languageChanged", updateTrayMenu);

    // 3. 组件卸载时，清理事件监听器，防止内存泄漏
    return () => {
      i18n.off("languageChanged", updateTrayMenu);
    };
  }, [i18n, t]);
  // 这个组件不渲染任何内容
  return null;
};

export default AppEventManager;
