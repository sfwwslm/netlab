import { format } from "date-fns";

/**
 * @function getFormattedTimestamp
 * @description 获取当前日期和时间并格式化为 'YYYY-MM-DD HH:mm:ss.SSS' 字符串
 * @returns {string} 格式化后的时间戳字符串
 */
export const getFormattedTimestamp = (): string => {
  return format(new Date(), "yyyy-MM-dd HH:mm:ss.SSS");
};
