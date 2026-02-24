import { IconRefreshProvider } from "@/contexts/IconRefreshContext";

import LoadTestPage from "./tools/loadtest";

const Home: React.FC = () => {
  return (
    <IconRefreshProvider>
      <LoadTestPage />
    </IconRefreshProvider>
  );
};

export default Home;
