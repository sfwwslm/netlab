import { createContext, useContext, ReactNode, useEffect } from "react";
import useLocalStorage from "@/hooks/useLocalStorage";
import { ThemeProvider as StyledThemeProvider } from "styled-components";
import { lightTheme, darkTheme, Theme } from "@/styles/themes";
import { emit, listen } from "@tauri-apps/api/event";

type ThemeContextType = {
  toggleTheme: () => void;
  themeName: "light" | "dark";
};

const ThemeContext = createContext<ThemeContextType | undefined>(undefined);

export const AppThemeProvider = ({ children }: { children: ReactNode }) => {
  const [themeName, setThemeName] = useLocalStorage<"light" | "dark">(
    "appTheme",
    "dark",
  );

  const toggleTheme = () => {
    setThemeName((prev) => (prev === "light" ? "dark" : "light"));
  };

  const theme: Theme = themeName === "light" ? lightTheme : darkTheme;

  useEffect(() => {
    const unlistenPromise = listen<{ theme: "light" | "dark" }>(
      "app:theme-changed",
      (event) => {
        const next = event.payload?.theme;
        if (next && next !== themeName) {
          setThemeName(next);
        }
      },
    );
    return () => {
      unlistenPromise.then((unlisten) => unlisten());
    };
  }, [setThemeName, themeName]);

  useEffect(() => {
    emit("app:theme-changed", { theme: themeName });
  }, [themeName]);

  return (
    <ThemeContext.Provider value={{ toggleTheme, themeName }}>
      <StyledThemeProvider theme={theme}>{children}</StyledThemeProvider>
    </ThemeContext.Provider>
  );
};

export const useAppTheme = () => {
  const context = useContext(ThemeContext);
  if (!context) {
    throw new Error("useAppTheme must be used within an AppThemeProvider");
  }
  return context;
};
