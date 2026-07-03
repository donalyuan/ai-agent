import type { Metadata } from "next";
import "./styles.css";

export const metadata: Metadata = {
  title: "VEDIO-AGENT 视频工作台",
  description: "VEDIO-AGENT 视频工作台",
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
