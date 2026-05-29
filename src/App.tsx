import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

function App() {
  const [status, setStatus] = useState("...");
  useEffect(() => {
    invoke<string>("health").then(setStatus).catch((e) => setStatus(`error: ${e}`));
  }, []);
  return <div className="p-4">backend health: {status}</div>;
}

export default App;
