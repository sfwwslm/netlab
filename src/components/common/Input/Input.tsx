import React, { forwardRef } from "react";
import InputBase from "./Input.styles";

type InputProps = React.InputHTMLAttributes<HTMLInputElement> & {
  disableWheel?: boolean;
};

const Input = forwardRef<HTMLInputElement, InputProps>(
  ({ disableWheel = true, onWheel, type, ...rest }, ref) => {
    const handleWheel = (event: React.WheelEvent<HTMLInputElement>) => {
      if (disableWheel && type === "number") {
        event.currentTarget.blur();
      }
      onWheel?.(event);
    };

    return <InputBase ref={ref} type={type} onWheel={handleWheel} {...rest} />;
  },
);

Input.displayName = "Input";

export default Input;
