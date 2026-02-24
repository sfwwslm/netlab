import { TFunction } from "i18next";

/**
 * 定义菜单项的类型
 */
export type MenuItem = {
  id: number;
  label: string;
  url: string; // 菜单项对应的URL
  children?: MenuItem[]; // 可选的子菜单项，实现嵌套
};

/**
 * 菜单项数据生成函数
 * @param t - i18next 的翻译函数
 * @returns {MenuItem[]}
 */
export const getMenuItems = (t: TFunction): MenuItem[] => [
  {
    id: 100,
    label: t("menu.tools.title"),
    url: "/",
    children: [
      {
        id: 101,
        label: t("menu.tools.loadTest"),
        url: "/tools/loadtest",
      },
      {
        id: 102,
        label: t("menu.tools.networkDebug"),
        url: "/tools/network",
      },
      {
        id: 103,
        label: t("menu.tools.proxy"),
        url: "/tools/proxy",
      },
      {
        id: 104,
        label: t("menu.tools.bytesConverter"),
        url: "/tools/bytes",
      },
    ],
  },
  {
    id: 900,
    label: t("menu.log"),
    url: "/logs",
  },
  {
    id: 1000,
    label: t("menu.help.title"),
    url: "/help",
    children: [
      {
        id: 1001,
        label: t("menu.help.settings"),
        url: "/help/settings",
      },
      {
        id: 1002,
        label: t("menu.help.changelog"),
        url: "/help/changelog",
      },
      // {
      //   id: 1003,
      //   label: t("menu.help.checkUpdate"),
      //   url: "/help/checkUpdate",
      // },
      {
        id: 1099,
        label: t("menu.help.about"),
        url: "/help/about",
      },
    ],
  },
];
