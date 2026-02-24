import React, { useState, useRef, useEffect } from "react";
import {
  SelectContainer,
  SelectedValue,
  ArrowIcon,
  OptionsList,
  OptionItem,
} from "./CustomSelect.styles";
import { AnimatePresence } from "framer-motion";

export interface SelectOption {
  value: string | number;
  label: string;
  disabled?: boolean; // 是否可被禁用
}

interface CustomSelectProps {
  id?: string;
  options: SelectOption[];
  value: string | number | undefined;
  onChange: (value: string | number) => void;
  placeholder?: string;
  disabled?: boolean;
}

const CustomSelect: React.FC<CustomSelectProps> = ({
  id,
  options,
  value,
  onChange,
  placeholder = "请选择...",
  disabled = false,
}) => {
  const [isOpen, setIsOpen] = useState(false);
  const selectRef = useRef<HTMLDivElement>(null);

  const selectedOption = options.find((opt) => opt.value === value);

  // 点击外部关闭下拉列表
  useEffect(() => {
    const handleClickOutside = (event: MouseEvent) => {
      if (
        selectRef.current &&
        !selectRef.current.contains(event.target as Node)
      ) {
        setIsOpen(false);
      }
    };
    document.addEventListener("mousedown", handleClickOutside);
    return () => {
      document.removeEventListener("mousedown", handleClickOutside);
    };
  }, []);

  const handleOptionClick = (option: SelectOption) => {
    // 如果选项被禁用，则不执行任何操作
    if (option.disabled) {
      return;
    }
    onChange(option.value);
    setIsOpen(false);
  };

  return (
    <SelectContainer ref={selectRef}>
      <SelectedValue
        id={id}
        type="button"
        $isOpen={isOpen}
        $disabled={disabled}
        onClick={() => {
          if (!disabled) {
            setIsOpen(!isOpen);
          }
        }}
        onKeyDown={(event) => {
          if (disabled) return;
          if (event.key === "Enter" || event.key === " ") {
            event.preventDefault();
            setIsOpen((prev) => !prev);
          }
        }}
        aria-expanded={isOpen}
        aria-disabled={disabled}
      >
        <span>{selectedOption ? selectedOption.label : placeholder}</span>
        <ArrowIcon $isOpen={isOpen} $disabled={disabled} />
      </SelectedValue>

      <AnimatePresence>
        {isOpen && !disabled && (
          <OptionsList
            initial={{ opacity: 0, y: -10 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, y: -10 }}
            transition={{ duration: 0.2 }}
          >
            {options.map((option) => (
              <OptionItem
                key={option.value}
                $isSelected={value === option.value}
                $isDisabled={!!option.disabled} // 传递禁用状态
                onClick={() => handleOptionClick(option)}
              >
                {option.label}
              </OptionItem>
            ))}
          </OptionsList>
        )}
      </AnimatePresence>
    </SelectContainer>
  );
};

export default CustomSelect;
