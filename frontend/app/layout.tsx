import type { Metadata } from "next";
import "./styles.css";

export const metadata: Metadata = {
  title: "Video Agent",
  description: "AI video generation agent workspace",
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
