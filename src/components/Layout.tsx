import { ReactNode } from "react";

interface LayoutProps {
  topBar: ReactNode;
  left: ReactNode;
  center: ReactNode;
  right: ReactNode;
}

export function Layout({ topBar, left, center, right }: LayoutProps) {
  return (
    <div className="flex h-screen flex-col">
      <header className="flex h-12 items-center gap-2 border-b px-3">{topBar}</header>
      <div className="flex flex-1 overflow-hidden">
        <aside className="w-[30%] overflow-auto border-r p-3">{left}</aside>
        <main className="w-[45%] overflow-auto p-3">{center}</main>
        <aside className="w-[25%] overflow-auto border-l p-3">{right}</aside>
      </div>
    </div>
  );
}
