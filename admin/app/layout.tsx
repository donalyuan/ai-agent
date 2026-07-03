import type { Metadata } from "next";
import "./styles.css";

export const metadata: Metadata = {
  title: "Novex 平台管理后台",
  description: "Novex 平台管理后台",
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
