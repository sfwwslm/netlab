/**
 * 定义内容片段类型，表示更新日志中的内容单元，可能是：
 * - 纯文本字符串，如普通描述文字
 * - 链接对象，包含显示文本和对应 URL
 * - 加粗文本（由 Markdown 行内代码 `code` 或 `**bold**` 转换而来）
 */
export type ContentSegment =
  | string
  | { linkText: string; url: string }
  | { boldText: string }
  | { codeText: string };

/**
 * 更新日志中的一条条目，包含：
 * - content：条目内容的片段数组，每个片段是 ContentSegment 类型
 * - level：该条目所在的层级（相对于当前节点层级偏移，通常用于渲染缩进）
 * - type: 标识该条目的类型，是普通列表项还是水平分割线
 */
export interface ChangelogItem {
  content: ContentSegment[];
  level: number;
  /** @description 条目类型，'item' 为普通列表项，'hr' 为水平分割线 */
  type: "item" | "hr";
}

/**
 * 更新日志树的节点，包含：
 * - title：该节点的标题（对应 Markdown 的标题文本）
 * - level：标题等级，范围 1～6（对应 Markdown 的 # 数量）
 * - items：该节点下的更新条目列表
 * - children：子节点数组，构成树状结构
 */
export interface ChangelogNode {
  title: string;
  level: number;
  items: ChangelogItem[];
  children: ChangelogNode[];
}

/**
 * 解析 Markdown 格式的更新日志内容，转换成层级树状结构。
 * - 忽略一级标题（#）通常是文档顶层标题
 * - 支持标题等级 2～6 (# 到 ######) 构建树形目录
 * - 支持解析条目中的 Markdown 链接 [text](url)
 * - 支持将行内代码 `code` 和 `**bold**` 转换为加粗文本 { boldText }
 * - 支持将 `---` 或更多 `-` 组成的行解析为水平分割线
 *
 * @param content - 原始 Markdown 格式的更新日志文本
 * @returns 解析后的根节点数组（除去虚拟根节点）
 */
export const parseChangelogTree = (content: string): ChangelogNode[] => {
  if (!content) return [];

  // 按行分割内容，兼容 Windows 和 Unix 换行符
  const lines = content.split(/\r?\n/);

  // 虚拟根节点，用于统一管理所有一级子节点
  const root: ChangelogNode = {
    title: "__root__",
    level: 0,
    items: [],
    children: [],
  };
  // 栈用于追踪当前层级，初始为根节点
  const stack: ChangelogNode[] = [root];

  // 匹配链接、`行内代码`、**加粗**
  const tokenRegex = /(\[([^\]]+)\]\(([^)]+)\))|`([^`]+)`|\*\*(.+?)\*\*/g;

  for (const rawLine of lines) {
    if (!rawLine.trim()) continue;

    // 检测标题行，匹配 # 号及后面标题文本
    const headingMatch = /^(#{1,6})\s+(.*)$/.exec(rawLine.trim());
    if (headingMatch) {
      const level = headingMatch[1].length; // # 的数量即标题等级
      const title = headingMatch[2].trim(); // 标题文本

      // 跳过一级标题，避免把文档最顶层标题加入树
      if (level === 1) continue;

      // 构造新的节点对象
      const newNode: ChangelogNode = { title, level, items: [], children: [] };

      // 从栈中弹出所有层级大于等于当前节点的节点，找到合适的父节点
      while (stack.length > 1 && stack[stack.length - 1].level >= level) {
        stack.pop();
      }

      // 将新节点加入当前栈顶节点的子节点列表
      stack[stack.length - 1].children.push(newNode);

      // 新节点入栈，表示后续内容属于该节点或其子节点
      stack.push(newNode);
      continue;
    }

    // 非标题行，认为是当前节点下的内容条目
    const currentNode = stack[stack.length - 1];

    // 修复：放宽水平线规则（三个或以上-），并正确设置条目类型
    if (/^-{3,}$/.test(rawLine.trim())) {
      currentNode.items.push({
        content: [], // 水平线没有内容
        level: currentNode.level,
        type: "hr", // 明确类型
      });
      continue;
    }

    const listMatch = /^\s*-\s+/.exec(rawLine);
    const isListItem = !!listMatch;

    // 去除列表标记前缀后得到纯文本行，用于后续解析
    // 注意不替换行内代码中的反引号
    const cleanedLine = isListItem
      ? rawLine.replace(/^\s*-\s+/, "")
      : rawLine.trim();

    const contentSegments: ContentSegment[] = [];
    let lastIndex = 0;
    let match: RegExpExecArray | null;

    tokenRegex.lastIndex = 0; // 重置正则匹配位置，从行首开始匹配

    // 遍历所有匹配到的链接或代码标记
    while ((match = tokenRegex.exec(cleanedLine)) !== null) {
      // 匹配到标记前的普通文本部分
      if (match.index > lastIndex) {
        const plain = cleanedLine.substring(lastIndex, match.index);
        if (plain.length) contentSegments.push(plain);
      }

      if (match[1]) {
        // 链接匹配组，match[2] 是链接文本，match[3] 是链接地址
        contentSegments.push({ linkText: match[2], url: match[3] });
      } else if (match[4]) {
        contentSegments.push({ boldText: match[4] }); // `code`
      } else if (match[5]) {
        contentSegments.push({ boldText: match[5] }); // **bold**
      }

      // 更新游标位置，继续搜索后续标记
      lastIndex = match.index + match[0].length;
    }

    // 处理行尾剩余的普通文本（如果有）
    if (lastIndex < cleanedLine.length) {
      const tail = cleanedLine.substring(lastIndex);
      if (tail.length) contentSegments.push(tail);
    }

    // 合并相邻的纯文本片段，避免文本被分割成多个字符串，提升渲染简洁性
    const mergedSegments: ContentSegment[] = [];
    for (const seg of contentSegments) {
      if (
        typeof seg === "string" &&
        mergedSegments.length > 0 &&
        typeof mergedSegments[mergedSegments.length - 1] === "string"
      ) {
        mergedSegments[mergedSegments.length - 1] =
          (mergedSegments[mergedSegments.length - 1] as string) + seg;
      } else {
        mergedSegments.push(seg);
      }
    }

    // 将解析后的条目添加到当前节点的 items 列表中
    // level 根据是否为列表项增加层级，用于显示缩进
    currentNode.items.push({
      content: mergedSegments,
      level: currentNode.level + (isListItem ? 1 : 0),
      type: "item", // 明确类型
    });
  }

  // 返回虚拟根节点的所有子节点，即顶层更新日志节点列表
  return root.children;
};
