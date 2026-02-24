import {
  createContext,
  useState,
  useCallback,
  useContext,
  ReactNode,
} from "react";

/**
 * @interface IconRefreshContextType
 * @description 定义 Context 的类型
 */
interface IconRefreshContextType {
  iconRetryKey: number;
  triggerIconRefresh: () => void;
}

const IconRefreshContext = createContext<IconRefreshContextType | undefined>(
  undefined,
);

/**
 * @component IconRefreshProvider
 * @description 提供 iconRetryKey 状态和刷新函数的 Provider 组件
 */
export const IconRefreshProvider = ({ children }: { children: ReactNode }) => {
  const [iconRetryKey, setIconRetryKey] = useState(0);

  /**
   * @function triggerIconRefresh
   * @description 触发所有图标刷新的回调函数
   * @description 使用 useCallback 包装以确保函数引用在多次渲染之间保持稳定
   */
  const triggerIconRefresh = useCallback(() => {
    setIconRetryKey((key) => key + 1);
  }, []);

  return (
    <IconRefreshContext.Provider value={{ iconRetryKey, triggerIconRefresh }}>
      {children}
    </IconRefreshContext.Provider>
  );
};

/**
 * @hook useIconRefresh
 * @description 方便组件消费 IconRefreshContext 的自定义 Hook
 */
export const useIconRefresh = () => {
  const context = useContext(IconRefreshContext);
  if (!context) {
    throw new Error(
      "useIconRefresh must be used within an IconRefreshProvider",
    );
  }
  return context;
};
