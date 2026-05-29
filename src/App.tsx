import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { ProgressPayload } from "@/types";

function App() {
  const [status, setStatus] = useState("...");
  const [progress, setProgress] = useState("");

  useEffect(() => {
    invoke<string>("health").then(setStatus);
    const un = listen<ProgressPayload>("progress", (e) => {
      setProgress(`${e.payload.current}/${e.payload.total} ${e.payload.message}`);
    });
    return () => { un.then((f) => f()); };
  }, []);

  return (
    <div className="p-4 space-y-2">
      <div>backend health: {status}</div>
      <button className="border px-2 py-1" onClick={() => invoke("demo_progress")}>
        run demo
      </button>
      <div>progress: {progress}</div>
    </div>
  );
}

export default App;
