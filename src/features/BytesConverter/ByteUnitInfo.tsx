import React from "react";
import styled from "styled-components";
import { FormGrid, Label } from "./BytesConverter.styles";
import { Theme } from "@/styles/themes";
import { useTranslation } from "react-i18next";

/**
 * 定义样式组件
 */
const InfoDescription = styled.p<{ theme: Theme }>`
  width: 100%;
  text-align: left;
  margin-bottom: calc(${(props) => props.theme.spacing.unit} * 3);
  color: ${(props) => props.theme.colors.textPrimary};
`;

const InfoInputGroup = styled.div<{ theme: Theme; $isLast: boolean }>`
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  margin-bottom: calc(${(props) => props.theme.spacing.unit} * 2);
  width: 100%;
  border-bottom: ${(props) =>
    props.$isLast ? "none" : `1px dashed ${props.theme.colors.border}`};
  padding-bottom: calc(${(props) => props.theme.spacing.unit} * 1.5);

  ${Label} {
    margin-bottom: calc(${(props) => props.theme.spacing.unit} / 2);
    font-size: 1.2rem;
    color: ${(props) => props.theme.colors.primary};
    font-weight: bold;
  }

  p {
    color: ${(props) => props.theme.colors.textSecondary};
    font-size: 0.95rem;
    line-height: 1.6;
  }
`;

const ByteUnitInfo: React.FC = () => {
  const { t } = useTranslation();
  /**
   * 字节单位数据
   */
  const byteUnitsData = [
    {
      name: t("tools.bytesConverter.unitNames.bit"),
      abbreviation: "bit (b)",
      description: t("tools.bytesConverter.unitInfoDescriptions.bit"),
    },
    {
      name: t("tools.bytesConverter.unitNames.byte"),
      abbreviation: "Byte (B)",
      description: t("tools.bytesConverter.unitInfoDescriptions.byte"),
    },
    {
      name: t("tools.bytesConverter.unitNames.kilobyte"),
      abbreviation: "Kilobyte (KB)",
      description: t("tools.bytesConverter.unitInfoDescriptions.kilobyte"),
    },
    {
      name: t("tools.bytesConverter.unitNames.megabyte"),
      abbreviation: "Megabyte (MB)",
      description: t("tools.bytesConverter.unitInfoDescriptions.megabyte"),
    },
    {
      name: t("tools.bytesConverter.unitNames.gigabyte"),
      abbreviation: "Gigabyte (GB)",
      description: t("tools.bytesConverter.unitInfoDescriptions.gigabyte"),
    },
    {
      name: t("tools.bytesConverter.unitNames.terabyte"),
      abbreviation: "Terabyte (TB)",
      description: t("tools.bytesConverter.unitInfoDescriptions.terabyte"),
    },
    {
      name: t("tools.bytesConverter.unitNames.petabyte"),
      abbreviation: "Petabyte (PB)",
      description: t("tools.bytesConverter.unitInfoDescriptions.petabyte"),
    },
    {
      name: t("tools.bytesConverter.unitNames.exabyte"),
      abbreviation: "Exabyte (EB)",
      description: t("tools.bytesConverter.unitInfoDescriptions.exabyte"),
    },
    {
      name: t("tools.bytesConverter.unitNames.zettabyte"),
      abbreviation: "Zettabyte (ZB)",
      description: t("tools.bytesConverter.unitInfoDescriptions.zettabyte"),
    },
    {
      name: t("tools.bytesConverter.unitNames.yottabyte"),
      abbreviation: "Yottabyte (YB)",
      description: t("tools.bytesConverter.unitInfoDescriptions.yottabyte"),
    },
  ];
  return (
    <FormGrid
      style={{
        display: "flex", // 使用 flex 布局，使内容垂直排列
        flexDirection: "column",
        alignItems: "flex-start", // 左对齐
      }}
    >
      <InfoDescription>{t("tools.bytesConverter.infoTitle")}</InfoDescription>
      {byteUnitsData.map((unit, index) => (
        <InfoInputGroup
          key={unit.abbreviation}
          $isLast={index === byteUnitsData.length - 1}
        >
          <Label>
            {unit.name} ({unit.abbreviation})
          </Label>
          <p>{unit.description}</p>
        </InfoInputGroup>
      ))}
    </FormGrid>
  );
};

export default ByteUnitInfo;
