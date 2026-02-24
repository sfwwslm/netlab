import React, { useState, useCallback, JSX } from "react";
import {
  ConverterContainer,
  Title,
  Description,
  FormGrid,
  FormGroup,
  Label,
  Input,
  CopyButton,
  ResetButtonContainer,
  ModeSwitchContainer,
  ModeSwitchButton,
} from "./BytesConverter.styles";
import {
  IoCalculatorOutline,
  IoCopyOutline,
  IoRefreshCircleOutline,
} from "react-icons/io5";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import ByteUnitInfo from "./ByteUnitInfo";
import { useTranslation } from "react-i18next";
import Tooltip from "@/components/common/Tooltip/Tooltip";

/** 定义字节单位类型 */
type UnitName =
  | "bit"
  | "byte"
  | "kilobyte"
  | "megabyte"
  | "gigabyte"
  | "terabyte";

const conversionRatesBinary = {
  bit: 1,
  byte: 8,
  kilobyte: 8 * 1024,
  megabyte: 8 * 1024 ** 2,
  gigabyte: 8 * 1024 ** 3,
  terabyte: 8 * 1024 ** 4,
};

/**
 * @function convertUnits
 * @description 根据选定的单位制进行单位转换（现在仅支持二进制）
 * @param value 输入的数值
 * @param from 原始单位
 * @returns 转换后的所有单位的数值对象
 */
function convertUnits(
  value: number,
  from: UnitName,
): Record<UnitName, number | string> {
  const rates = conversionRatesBinary; // 总是使用二进制转换率

  const bitValue = value * rates[from];

  const round = (num: number) => {
    if (isNaN(num) || !isFinite(num)) return ""; // 处理NaN和Infinity
    if (num === 0) return 0;
    // 使用科学计数法处理非常小或非常大的数字，否则保留6位小数
    if (Math.abs(num) < 0.000001 || Math.abs(num) > 1e15) {
      return Number(num.toExponential(6));
    }
    return Number(num.toFixed(6));
  };

  return {
    bit: round(bitValue / rates.bit),
    byte: round(bitValue / rates.byte),
    kilobyte: round(bitValue / rates.kilobyte),
    megabyte: round(bitValue / rates.megabyte),
    gigabyte: round(bitValue / rates.gigabyte),
    terabyte: round(bitValue / rates.terabyte),
  };
}

// 定义模式类型
type ConverterMode = "unitConversion" | "byteUnitInfo";

const BytesConverter: React.FC = () => {
  const { t } = useTranslation();
  const [values, setValues] = useState<Record<string, number | string>>({});
  const [mode, setMode] = useState<ConverterMode>("unitConversion"); // 默认单位转换模式

  /**
   * 定义单位字段及其标签（现在仅用于二进制转换模式）
   */
  const unitFields: { label: string; name: UnitName; icon: JSX.Element }[] = [
    {
      label: t("tools.bytesConverter.units.bit"),
      name: "bit",
      icon: <IoCalculatorOutline />,
    },
    {
      label: t("tools.bytesConverter.units.byte"),
      name: "byte",
      icon: <IoCalculatorOutline />,
    },
    {
      label: t("tools.bytesConverter.units.kilobyte"),
      name: "kilobyte",
      icon: <IoCalculatorOutline />,
    },
    {
      label: t("tools.bytesConverter.units.megabyte"),
      name: "megabyte",
      icon: <IoCalculatorOutline />,
    },
    {
      label: t("tools.bytesConverter.units.gigabyte"),
      name: "gigabyte",
      icon: <IoCalculatorOutline />,
    },
    {
      label: t("tools.bytesConverter.units.terabyte"),
      name: "terabyte",
      icon: <IoCalculatorOutline />,
    },
  ];

  /**
   * @function handleChange
   * @description 处理输入框数值变化的事件
   * @param e 输入事件对象
   * @param name 改变的单位名称
   */
  const handleChange = useCallback(
    (e: React.ChangeEvent<HTMLInputElement>, name: UnitName) => {
      const val = parseFloat(e.target.value);
      if (!isNaN(val)) {
        const newValues = convertUnits(val, name);
        setValues(newValues);
      } else {
        setValues({});
      }
    },
    [],
  );

  /**
   * @function handleUnitConversionReset
   * @description 重置所有输入框 (针对单位转换模式)
   */
  const handleUnitConversionReset = useCallback(() => {
    setValues({});
  }, []);

  /**
   * @function handleCopy
   * @description 复制单个单位的结果到剪贴板
   * @param unit 复制的单位名称
   */
  const handleCopy = useCallback(
    async (unit: UnitName) => {
      const textToCopy = values[unit]?.toString() || "";
      if (textToCopy) {
        await writeText(textToCopy);
        console.log(`Copied ${unit}: ${textToCopy}`);
      }
    },
    [values],
  );

  return (
    <ConverterContainer>
      <Title>{t("tools.bytesConverter.title")}</Title>
      <Description>{t("tools.bytesConverter.description")}</Description>
      {/* 模式切换按钮 */}
      <ModeSwitchContainer>
        <ModeSwitchButton
          $isActive={mode === "unitConversion"}
          onClick={() => setMode("unitConversion")}
          whileTap={{ scale: 0.95 }}
        >
          {t("tools.bytesConverter.unitConversion")}
        </ModeSwitchButton>
        <ModeSwitchButton
          $isActive={mode === "byteUnitInfo"}
          onClick={() => setMode("byteUnitInfo")}
          whileTap={{ scale: 0.95 }}
        >
          {t("tools.bytesConverter.unitInfo")}
        </ModeSwitchButton>
      </ModeSwitchContainer>
      {/* 根据模式渲染不同的内容 */}
      {mode === "unitConversion" && (
        <FormGrid>
          <ResetButtonContainer>
            <Tooltip text={t("button.reset")}>
              <IoRefreshCircleOutline onClick={handleUnitConversionReset} />
            </Tooltip>
          </ResetButtonContainer>

          {unitFields.map(({ label, name, icon }) => (
            <FormGroup key={name}>
              <Label htmlFor={name}>
                {icon}
                {label}:
              </Label>
              <Input
                id={name}
                name={name}
                type="number"
                value={values[name] ?? ""}
                onChange={(e) => handleChange(e, name)}
                min={0}
                placeholder={t("tools.bytesConverter.inputPlaceholder")}
              />
              {values[name] !== "" && values[name] !== undefined && (
                <CopyButton onClick={() => handleCopy(name)}>
                  <IoCopyOutline />
                </CopyButton>
              )}
            </FormGroup>
          ))}
        </FormGrid>
      )}
      {mode === "byteUnitInfo" && <ByteUnitInfo />}
    </ConverterContainer>
  );
};

export default BytesConverter;
