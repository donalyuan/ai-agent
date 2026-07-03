import type { Metadata } from "next";
import "./styles.css";

export const metadata: Metadata = {
  title: "AI-AGENT 智能体工作台",
  description: "AI-AGENT 智能体工作台",
};

export default function RootLayout({
  children,
}: Readonly<{
  children: React.ReactNode;
}>) {
  return (
    <html lang="zh-CN">
      <body>{children}</body>
    </html>
  );
}
