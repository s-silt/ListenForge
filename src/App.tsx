import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Layout } from "@/components/Layout";

function App() {
  const [status, setStatus] = useState("...");

  useEffect(() => {
    invoke<string>("health").then(setStatus);
  }, []);

  return (
    <Layout
      topBar={
        <>
          <span className="font-medium">ListenForge</span>
          <span className="ml-auto text-xs text-muted-foreground">backend: {status}</span>
        </>
      }
      left={<div className="text-sm text-muted-foreground">文件预览 / 识别结果(待 M1)</div>}
      center={<div className="text-sm text-muted-foreground">听力稿编辑区(待 M3)</div>}
      right={<div className="text-sm text-muted-foreground">语音 & 导出设置(待 M4)</div>}
    />
  );
}

export default App;
