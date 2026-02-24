import React, { useMemo, JSX } from "react";
import styled from "styled-components";
import { useTranslation } from "react-i18next";
import { parseChangelogTree } from "@/utils";
import type { ChangelogNode, ChangelogItem, ContentSegment } from "@/utils";
import { openUrl } from "@tauri-apps/plugin-opener";

/**
 * 外层容器
 * - 占满整个可用区域
 * - 提供内边距与滚动支持
 * - 颜色和背景跟随主题
 */
const ChangelogContainer = styled.div`
  width: 100%;
  height: 100%;
  padding: 2rem;
  box-sizing: border-box;
  overflow-y: auto;
  color: ${(props) => props.theme.colors.textPrimary};
  background-color: ${(props) => props.theme.colors.background};
`;

/**
 * 页面主标题（固定显示“变更日志”）
 */
const Title = styled.h1`
  color: ${(props) => props.theme.colors.primary};
  border-bottom: 2px solid ${(props) => props.theme.colors.border};
  padding-bottom: 0.5rem;
  margin-bottom: 1.5rem;
`;

/**
 * 版本标题 / 节点标题
 * - 根据标题层级动态缩进
 * - 仅二级标题显示左侧竖线
 * - 字体大小随层级递减
 */
const Heading = styled.div<{ level: number }>`
  font-weight: bold;
  color: ${(props) => props.theme.colors.textPrimary};
  margin-top: 1rem;
  margin-bottom: 0.5rem;
  padding-left: ${(props) => `${(props.level - 1) * 1}rem`};

  /* 二级标题添加左边框以突出版本区块 */
  border-left: ${(props) =>
    props.level === 2 ? `4px solid ${props.theme.colors.primary}` : "none"};

  font-size: ${({ level }) =>
    level === 1 ? "1.5rem" : level === 2 ? "1.3rem" : "1.1rem"};
`;

/**
 * 列表容器
 * - 去掉默认 list-style
 * - 缩进随层级变化
 */
const List = styled.ul<{ level: number }>`
  list-style-type: none;
  padding-left: ${(props) => `${props.level * 1}rem`};
`;

/**
 * 列表项
 * - 使用自定义符号（奇数层“•”，偶数层“◦”）
 * - 左侧绝对定位的符号颜色跟随主题主色
 */
const ListItem = styled.li<{ level: number }>`
  margin-bottom: 0.5rem;
  line-height: 1.6;
  padding-left: 1rem;
  position: relative;

  &::before {
    content: "${(props) => (props.level % 2 === 0 ? "◦" : "•")}";
    position: absolute;
    left: 0;
    color: ${(props) => props.theme.colors.primary};
  }
`;

/**
 * 可点击的超链接样式
 * - 主色高亮
 * - 悬停时加下划线与亮度变化
 */
const ClickableLink = styled.span`
  color: ${(props) => props.theme.colors.primary};
  font-weight: 500;
  cursor: pointer;

  &:hover {
    text-decoration: underline;
    filter: brightness(1.1);
  }
`;

/**
 * 行内加粗（对应 Markdown 的 `code` 或 `**bold**`）
 * - 保留字体加粗
 * - 主色显示
 * - 添加轻微 padding 以突出
 */
const BoldInline = styled.strong`
  color: ${(props) => props.theme.colors.primary};
  font-weight: bold;
  padding: 0.1em 0.3em;
`;

/**
 * `行内代码` 的样式
 * - 使用等宽字体、不同的背景和颜色来模拟代码片段
 */
const CodeInline = styled.code`
  font-weight: bold;
  color: ${(props) => props.theme.colors.codeBackground};
  padding: 0.2em 0.4em;
  margin: 0;
  font-size: 85%; /* 通常比正文小一点 */
  border-radius: 6px;
`;

/**
 * 水平分割线样式
 */
const Separator = styled.hr`
  border: none;
  border-top: 1px solid ${(props) => props.theme.colors.border};
  margin: 1.5rem 0;
`;

/**
 * 辅助函数：渲染单个条目中的内容片段
 * @param segments - 内容片段数组
 */
const renderSegments = (segments: ContentSegment[]): (JSX.Element | null)[] => {
  return segments.map((segment, sIdx) => {
    if (typeof segment === "string") {
      return <span key={sIdx}>{segment}</span>;
    }
    if ("linkText" in segment) {
      return (
        <ClickableLink key={sIdx} onClick={() => openUrl(segment.url)}>
          {segment.linkText}
        </ClickableLink>
      );
    }
    // **加粗** 文本使用 BoldInline 组件
    if ("boldText" in segment) {
      return <BoldInline key={sIdx}>{segment.boldText}</BoldInline>;
    }
    // `行内代码` 使用 CodeInline 组件
    if ("codeText" in segment) {
      return <CodeInline key={sIdx}>{segment.codeText}</CodeInline>;
    }
    return null;
  });
};

/**
 * 渲染单个 Changelog 节点（递归）
 * @param node - 当前 changelog 节点（可能包含子标题与条目）
 */
const renderNode = (node: ChangelogNode): JSX.Element => {
  const content: JSX.Element[] = [];
  let currentListItems: ChangelogItem[] = [];

  // 辅助函数，用于将累积的列表项渲染到 VDOM 数组中
  const flushList = (key: string | number) => {
    if (currentListItems.length > 0) {
      content.push(
        <List key={`list-${key}`} level={node.level}>
          {currentListItems.map((listItem, listIdx) => (
            <ListItem
              key={`${node.title}-item-${key}-${listIdx}`}
              level={listItem.level}
            >
              {renderSegments(listItem.content)}
            </ListItem>
          ))}
        </List>,
      );
      currentListItems = []; // 清空累积的列表项
    }
  };

  // 遍历所有条目，将普通条目分组到 <List> 中，并独立渲染 <Separator>
  node.items.forEach((item, idx) => {
    if (item.type === "hr") {
      flushList(idx); // 渲染当前累积的列表
      content.push(<Separator key={`${node.title}-hr-${idx}`} />); // 渲染分割线
    } else {
      currentListItems.push(item); // 累积普通列表项
    }
  });

  flushList("final"); // 渲染循环结束后剩余的所有列表项

  return (
    <div key={node.title}>
      {/* 标题 */}
      <Heading level={node.level}>{node.title}</Heading>

      {/* 渲染已分组的内容（列表和分割线） */}
      {content}

      {/* 子标题递归渲染 */}
      {node.children.length > 0 &&
        node.children.map((child) => renderNode(child))}
    </div>
  );
};

/**
 * Changelog 页面组件
 * - 解析 changelog 文本为树状结构
 * - 递归渲染版本标题与内容
 */
const ChangelogPage: React.FC = () => {
  const { t } = useTranslation();

  // 仅在初始化时解析一次 changelog 内容
  const parsedTree = useMemo(
    () => parseChangelogTree(__CHANGELOG_CONTENT__).slice(0, 7),
    [],
  );

  return (
    <ChangelogContainer>
      <Title>{t("menu.help.changelog")}</Title>
      {parsedTree.map((node) => renderNode(node))}
    </ChangelogContainer>
  );
};

export default ChangelogPage;
